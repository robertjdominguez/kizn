//! A fake model server for tests and `just dev mock`. It speaks both wire
//! protocols (OpenAI chat completions and Anthropic Messages, streaming
//! included), so the real clients are exercised end to end without a model.
//!
//! The behaviour comes from the first path segment
//! (`http://127.0.0.1:PORT/no-json/v1`), or, for requests that arrive by
//! proxy for another host (`http://gpu:11434/v1`), from a host table that
//! defaults to `works`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behaviour {
    /// Schema-constrained JSON, streams.
    Works,
    /// Rejects a schema (400) but answers in JSON when asked.
    PromptedJson,
    /// Prose, whatever it's asked.
    NoJson,
    /// Answers, but a streamed request fails.
    NoStream,
    /// 401 on everything.
    Unauthorized,
    /// 404 "model not found" on completions.
    UnknownModel,
    /// Lists models, but refuses every request: no credit (Anthropic's 400).
    NoCredit,
    /// Writes items as asked, but the first one's correct option is far
    /// longer than the others (the longest-option cue).
    LongestCue,
    /// Answers blind checks with an option that isn't the key.
    CheckerDisagrees,
    /// Like `works`, but each kizn task takes a while (to watch it in dev).
    Slow,
}

impl Behaviour {
    pub const ALL: [Behaviour; 10] = [
        Behaviour::Works,
        Behaviour::PromptedJson,
        Behaviour::NoJson,
        Behaviour::NoStream,
        Behaviour::Unauthorized,
        Behaviour::UnknownModel,
        Behaviour::NoCredit,
        Behaviour::LongestCue,
        Behaviour::CheckerDisagrees,
        Behaviour::Slow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Behaviour::Works => "works",
            Behaviour::PromptedJson => "prompted-json",
            Behaviour::NoJson => "no-json",
            Behaviour::NoStream => "no-stream",
            Behaviour::Unauthorized => "unauthorized",
            Behaviour::UnknownModel => "unknown-model",
            Behaviour::NoCredit => "no-credit",
            Behaviour::LongestCue => "longest-cue",
            Behaviour::CheckerDisagrees => "checker-disagrees",
            Behaviour::Slow => "slow",
        }
    }

    fn from_name(s: &str) -> Option<Behaviour> {
        Behaviour::ALL.into_iter().find(|b| b.name() == s)
    }
}

#[derive(Clone, Default)]
pub struct FakeServer {
    addr: Option<SocketAddr>,
    hosts: Arc<Mutex<HashMap<String, Behaviour>>>,
    /// The key of every item this server wrote, by stem: the blind check
    /// answers from it.
    keys: Arc<Mutex<HashMap<String, String>>>,
    /// Varies what it writes, so stems don't repeat.
    written: Arc<std::sync::atomic::AtomicUsize>,
}

impl FakeServer {
    /// Starts on 127.0.0.1 (port 0 picks a free one).
    pub async fn start(port: u16) -> std::io::Result<FakeServer> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        let server = FakeServer { addr: Some(listener.local_addr()?), ..FakeServer::default() };
        let state = server.clone();
        tokio::spawn(async move {
            let app = axum::Router::new().fallback(handle).with_state(state);
            let _ = axum::serve(listener, app).await;
        });
        Ok(server)
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr.expect("started")
    }

    /// The base URL of an OpenAI-compatible endpoint with this behaviour.
    pub fn openai_url(&self, b: Behaviour) -> String {
        format!("http://{}/{}/v1", self.addr(), b.name())
    }

    /// The base URL of an Anthropic endpoint with this behaviour.
    pub fn anthropic_url(&self, b: Behaviour) -> String {
        format!("http://{}/{}", self.addr(), b.name())
    }

    /// How requests proxied for `host` behave.
    pub fn set_host(&self, host: &str, b: Behaviour) {
        self.hosts.lock().unwrap().insert(host.to_string(), b);
    }

    /// Model traffic for the app: every non-loopback request comes here
    /// (http or https), so URLs like `http://gpu:11434/v1` or
    /// `https://api.anthropic.com` reach the fake. Loopback addresses go
    /// direct, so a closed local port stays closed.
    pub fn client(&self) -> crate::Http {
        crate::Http::redirected(format!("http://{}", self.addr()))
    }
}

async fn handle(
    axum::extract::State(server): axum::extract::State<FakeServer>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path();
    // The fake web: pages sources are checked against (any host).
    if method == Method::GET && path.starts_with("/web/") {
        return web_page(path);
    }
    // The fake YouTube: watch pages and their caption tracks (any host).
    if method == Method::GET && (path == "/watch" || path == "/api/timedtext") {
        return youtube(path, uri.query().unwrap_or(""));
    }
    let (behaviour, rest) = match path.trim_start_matches('/').split_once('/') {
        Some((first, rest)) if Behaviour::from_name(first).is_some() => {
            (Behaviour::from_name(first).unwrap(), format!("/{rest}"))
        }
        _ => {
            let host = headers.get(crate::REDIRECT_HOST_HEADER).and_then(|h| h.to_str().ok()).map(str::to_string);
            let b = host.and_then(|h| server.hosts.lock().unwrap().get(&h).copied()).unwrap_or(Behaviour::Works);
            (b, path.to_string())
        }
    };
    if behaviour == Behaviour::Unauthorized {
        return error(StatusCode::UNAUTHORIZED, "invalid api key");
    }
    let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let task = kizn_task(&req);
    if task.is_some() && behaviour == Behaviour::Slow {
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    }
    let answer = task.and_then(|(task, input)| server.answer(task, &input, behaviour));
    match (method, rest.as_str()) {
        (Method::GET, "/v1/models") => axum::Json(json!({
            "object": "list",
            "data": [{"id": "fake-small"}, {"id": "fake-large"}, {"id": "qwen3:32b"}, {"id": "claude-sonnet-5"}]
        }))
        .into_response(),
        (Method::POST, "/v1/chat/completions") => openai(behaviour, &req, answer),
        (Method::POST, "/v1/messages") => anthropic(behaviour, &req, answer),
        _ => error(StatusCode::NOT_FOUND, "no such route"),
    }
}

/// Pages for checking suggested sources: a readable article about whatever
/// its slug says, an app shell, an off-topic article, and a 404.
fn web_page(path: &str) -> Response {
    let html = |status: StatusCode, body: String| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(body))
            .unwrap()
    };
    let rest = &path["/web/".len()..];
    match rest.split_once('/') {
        Some(("article", slug)) => {
            let about = percent_decode(slug).replace('-', " ");
            let para = format!(
                "<p>This page explains {about}. In short: {about}. It walks through an example, the common \
                 mix-ups people make about it, and how to check it yourself in a real system. Read it once \
                 slowly, then try explaining {about} to a colleague without looking.</p>"
            );
            html(
                StatusCode::OK,
                format!(
                    "<!doctype html><html><head><title>{about} | Example Docs</title><script>analytics()</script>\
                     </head><body><nav>Docs Guides Reference</nav><article><h1>{about}</h1>{}</article>\
                     <footer>Example Docs</footer></body></html>",
                    para.repeat(6)
                ),
            )
        }
        _ if rest == "shell" => html(
            StatusCode::OK,
            "<!doctype html><html><head><title>Learn</title></head><body><div id=\"root\"></div>\
             <script type=\"module\" src=\"/assets/app.js\"></script></body></html>"
                .into(),
        ),
        _ if rest == "offtopic" => html(
            StatusCode::OK,
            format!(
                "<html><head><title>Growing tomatoes</title></head><body><p>{}</p></body></html>",
                "Tomatoes need full sun, steady watering and a long warm summer to ripen well. ".repeat(30)
            ),
        ),
        _ => html(StatusCode::NOT_FOUND, "<html><body>Not found</body></html>".into()),
    }
}

/// A watch page (`v=captioned-…` has an English caption track, anything else
/// none) and its caption track, about whatever the id's slug says.
fn youtube(path: &str, query: &str) -> Response {
    let v = query.split('&').find_map(|kv| kv.strip_prefix("v=")).unwrap_or("").to_string();
    let about = v.split_once('-').map(|(_, slug)| slug.replace('-', " ")).unwrap_or_default();
    let reply = |content_type: &str, body: String| {
        Response::builder().header(header::CONTENT_TYPE, content_type).body(Body::from(body)).unwrap()
    };
    if path == "/api/timedtext" {
        let lines = [
            format!("today we look at {about}"),
            format!("in short, {about}"),
            "let's see it on a real system".to_string(),
            format!("here's the common mix-up about {about}"),
            "and here's how to check it yourself".to_string(),
        ];
        let cues: String = (0..40)
            .map(|i| format!(r#"<text start="{}.0" dur="4.0">{}</text>"#, i * 5, lines[i % lines.len()]))
            .collect();
        return reply(
            "text/xml; charset=utf-8",
            format!(r#"<?xml version="1.0" encoding="utf-8" ?><transcript>{cues}</transcript>"#),
        );
    }
    let captions = if v.starts_with("captioned-") {
        format!(
            r#""captions":{{"playerCaptionsTracklistRenderer":{{"captionTracks":[{{"baseUrl":"https://www.youtube.com/api/timedtext?v={v}&lang=en","languageCode":"en","name":{{"simpleText":"English"}}}}]}}}},"#
        )
    } else {
        String::new()
    };
    reply(
        "text/html; charset=utf-8",
        format!(
            r#"<!doctype html><html><head><title>{about} explained - YouTube</title></head><body><div id="player"></div>
<script>var ytInitialPlayerResponse = {{{captions}"videoDetails":{{"videoId":"{v}","title":"{about} explained"}}}};</script></body></html>"#
        ),
    )
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const NO_CREDIT: &str = "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.";

const ANSWER: &str = r#"{"ok": true, "word": "kizn"}"#;
const PROSE: &str = "Sure! I'd be happy to help with that. kizn works.";

fn error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(json!({"error": {"message": message}}))).into_response()
}

/// What a streamed reply says, word by word: a task's "text" (the tutor), or
/// "kizn works." (the probe).
fn streamed_words(answer: Option<&Value>) -> Vec<String> {
    match answer.and_then(|a| a["text"].as_str()) {
        Some(text) => text.split_inclusive(' ').map(str::to_string).collect(),
        None => vec!["kizn ".into(), "works.".into()],
    }
}

/// Server-sent events, a little apart when `paced` (so a streamed reply
/// arrives word by word, as a real model's does).
fn sse_paced(events: Vec<String>, paced: bool, slow: bool) -> Response {
    if !paced {
        return sse(events);
    }
    // The slow model streams slowly too (a word every 40 ms).
    let gap = std::time::Duration::from_millis(if slow { 40 } else { 12 });
    use futures_util::StreamExt;
    let stream = futures_util::stream::iter(events).then(move |e| async move {
        tokio::time::sleep(gap).await;
        Ok::<_, std::convert::Infallible>(Bytes::from(format!("data: {e}\n\n")))
    });
    Response::builder().header(header::CONTENT_TYPE, "text/event-stream").body(Body::from_stream(stream)).unwrap()
}

fn sse(events: Vec<String>) -> Response {
    let body: String = events.into_iter().map(|e| format!("data: {e}\n\n")).collect();
    Response::builder().header(header::CONTENT_TYPE, "text/event-stream").body(Body::from(body)).unwrap()
}

/// Whether the request asks for JSON in its prompt (Prompted mode).
fn asks_for_json(req: &Value) -> bool {
    req.to_string().contains("Reply with only a JSON object")
}

fn openai(b: Behaviour, req: &Value, answer: Option<Value>) -> Response {
    if b == Behaviour::NoCredit {
        return error(StatusCode::BAD_REQUEST, NO_CREDIT);
    }
    if b == Behaviour::UnknownModel {
        return error(StatusCode::NOT_FOUND, "model not found");
    }
    if req["stream"] == true {
        if b == Behaviour::NoStream {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "streaming is broken here");
        }
        let words = streamed_words(answer.as_ref());
        let mut events: Vec<String> =
            words.iter().map(|t| json!({"choices": [{"delta": {"content": t}}]}).to_string()).collect();
        events.push("[DONE]".into());
        return sse_paced(events, words.len() > 2, b == Behaviour::Slow);
    }
    let schema = !req["response_format"].is_null();
    let answer = answer.map(|a| a.to_string()).unwrap_or_else(|| ANSWER.to_string());
    let content = match (b, schema) {
        (Behaviour::PromptedJson, true) => return error(StatusCode::BAD_REQUEST, "response_format is not supported"),
        (Behaviour::NoJson, _) => PROSE.to_string(),
        (_, true) => answer,
        (_, false) if asks_for_json(req) => format!("Here you go:\n```json\n{answer}\n```"),
        (_, false) => PROSE.to_string(),
    };
    axum::Json(json!({
        "choices": [{"message": {"role": "assistant", "content": content}}],
        "usage": {"prompt_tokens": 20, "completion_tokens": 8}
    }))
    .into_response()
}

fn anthropic(b: Behaviour, req: &Value, answer: Option<Value>) -> Response {
    if b == Behaviour::NoCredit {
        return error(StatusCode::BAD_REQUEST, NO_CREDIT);
    }
    if b == Behaviour::UnknownModel {
        return error(StatusCode::NOT_FOUND, "model: not found");
    }
    if req["stream"] == true {
        if b == Behaviour::NoStream {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "streaming is broken here");
        }
        let words = streamed_words(answer.as_ref());
        let mut events = vec![json!({"type": "message_start"}).to_string()];
        for t in &words {
            events.push(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": t}}).to_string());
        }
        events.push(json!({"type": "message_stop"}).to_string());
        return sse_paced(events, words.len() > 2, b == Behaviour::Slow);
    }
    let tools = !req["tools"].is_null();
    let answer = answer.unwrap_or_else(|| json!({"ok": true, "word": "kizn"}));
    let content = match (b, tools) {
        (Behaviour::PromptedJson, true) => return error(StatusCode::BAD_REQUEST, "tools are not supported"),
        (Behaviour::NoJson, _) => json!([{"type": "text", "text": PROSE}]),
        (_, true) => json!([{"type": "tool_use", "name": "reply", "input": answer}]),
        (_, false) if asks_for_json(req) => json!([{"type": "text", "text": answer.to_string()}]),
        (_, false) => json!([{"type": "text", "text": PROSE}]),
    };
    axum::Json(json!({"content": content, "usage": {"input_tokens": 20, "output_tokens": 8}})).into_response()
}

// ---- kizn's tasks ---------------------------------------------------------------
//
// kizn's prompts carry `kizn-task: <task>` in the system prompt and their
// inputs as JSON between <input> and </input> in the user message. The fake
// answers each with valid JSON built from those inputs.

pub const TASK_MARKER: &str = "kizn-task:";

fn kizn_task(req: &Value) -> Option<(String, Value)> {
    let text = req.to_string();
    let at = text.find(TASK_MARKER)? + TASK_MARKER.len();
    let task: String = text[at..].trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    // The user message, as text (OpenAI puts it in messages; Anthropic too).
    let user = req["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())?
        .to_string();
    let start = user.find("<input>")? + "<input>".len();
    let end = user.find("</input>")?;
    Some((task, serde_json::from_str(user[start..end].trim()).ok()?))
}

const VERBS: [(&str, &str); 5] = [
    ("Explain", "understand"),
    ("Diagnose", "analyze"),
    ("Advise", "evaluate"),
    ("Compare", "analyze"),
    ("Design", "create"),
];

const STEMS: [&str; 6] = [
    "Which statement about {c} is accurate?",
    "A colleague asks you about {c}. What do you tell them?",
    "During an incident review someone brings up {c}. Which point holds?",
    "You are writing onboarding notes covering {c}. Which line belongs there?",
    "A customer email raises {c}. Which reply gets it right?",
    "In a design discussion touching {c}, which claim stands up?",
];

/// Varied settings, so stems written for the same concepts don't overlap.
const CONTEXTS: [&str; 12] = [
    "Picture a staging cluster after a Friday deploy.",
    "Think of a laptop running a quick local test.",
    "Imagine a nightly batch job on shared hardware.",
    "Consider a payments service under heavy load.",
    "Suppose an intern follows the runbook step by step.",
    "Assume a regulated bank audits every release.",
    "Take a startup moving fast with one engineer.",
    "Envision a mirror registry behind a strict firewall.",
    "Recall a rollback that happened mid-incident.",
    "Visualise dashboards showing a sudden latency spike.",
    "Consider vendors shipping images to many customers.",
    "Imagine edge devices updating over flaky links.",
];
const AUDIENCES: [&str; 6] = [
    "a new teammate",
    "a customer's architect",
    "a product manager",
    "an auditor",
    "a support engineer",
    "a skeptical director",
];
const SITUATIONS: [&str; 5] = [
    "has asked for a plain answer",
    "is deciding whether to approve a release",
    "is reading an incident report",
    "is comparing two vendors",
    "has five minutes before a meeting",
];

impl FakeServer {
    fn next(&self) -> usize {
        self.written.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    fn answer(&self, task: String, input: &Value, b: Behaviour) -> Option<Value> {
        let s = |k: &str| input[k].as_str().unwrap_or("").to_string();
        match task.as_str() {
            "draft-outcomes" => {
                let (title, goal) = (s("title"), s("goal"));
                let outcomes: Vec<Value> = VERBS[..3]
                    .iter()
                    .enumerate()
                    .map(|(i, (verb, tier))| {
                        json!({
                            "statement": format!("{verb} {} for the situations in your goal (part {})", title.to_lowercase(), i + 1),
                            "tier": tier,
                            "reason": format!("you said \"{goal}\""),
                        })
                    })
                    .collect();
                Some(json!({ "outcomes": outcomes }))
            }
            "draft-concepts" => {
                let outcome = s("outcome");
                let short: String = outcome.split_whitespace().skip(1).take(3).collect::<Vec<_>>().join(" ");
                let concepts: Vec<Value> = ["what it is", "why it matters", "how it fails", "how to tell"]
                    .iter()
                    .map(|aspect| json!(format!("{short}: {aspect}")))
                    .collect();
                Some(json!({ "concepts": concepts }))
            }
            "write-items" => {
                let kind = s("kind");
                // "remember or understand": a real model picks one.
                // The last tier the request allows (each is a single tier).
                let tier = input["tiers"]
                    .as_array()
                    .and_then(|t| t.last())
                    .and_then(|t| t.as_str())
                    .unwrap_or("understand")
                    .to_string();
                let count = input["count"].as_u64().unwrap_or(1) as usize;
                let concepts: Vec<String> = input["concepts"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let named: Vec<String> = match input["slot_concept"].as_str() {
                    Some(c) => vec![c.to_string()],
                    None => concepts.iter().take(2).cloned().collect(),
                };
                // Written from a resource when one is given: cite it.
                let cite = input["resources"][0]["title"].as_str();
                let items: Vec<Value> = (0..count)
                    .map(|i| {
                        let n = self.next();
                        let about = named.join(" and ");
                        if kind == "explain" {
                            return json!({
                                "concepts": named,
                                "stem": format!("Explain {about} to {} who {}.", AUDIENCES[n % AUDIENCES.len()], SITUATIONS[(n / AUDIENCES.len()) % SITUATIONS.len()]),
                                "exemplar": format!("A strong answer names {about}, says why it matters in practice, and gives one concrete example."),
                                "source": cite.unwrap_or(""),
                                "model_knowledge": cite.is_none(),
                            });
                        }
                        let stem = format!(
                            "{} {}",
                            CONTEXTS[(n + n / STEMS.len()) % CONTEXTS.len()],
                            STEMS[n % STEMS.len()].replace("{c}", &about)
                        );
                        let key = if b == Behaviour::LongestCue && i == 0 {
                            "This is how it works in practice, every time, on every host, whatever the registry or tag says about it"
                        } else {
                            "This is how it works in practice"
                        };
                        self.keys.lock().unwrap().insert(stem.clone(), key.to_string());
                        json!({
                            "concepts": named,
                            "tier": tier,
                            "stem": stem,
                            "options": [
                                {"text": key, "correct": true, "why": "it matches the concept as stated"},
                                {"text": "This only applies on the first run", "correct": false, "why": "nothing limits it to the first run", "misconception": "it happens once", "foundation": "none"},
                                {"text": "This depends on the host's clock", "correct": false, "why": "time plays no part here", "misconception": "it is time-based", "foundation": "none"},
                            ],
                            "source": cite.unwrap_or(""),
                            "model_knowledge": cite.is_none(),
                        })
                    })
                    .collect();
                Some(json!({ "items": items }))
            }
            "draft-foundations" => {
                // The same three each time, so a scenario can have one of
                // them already (named exactly as listed, as asked).
                let yours: Vec<String> = input["your_topics"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let named = |want: &str| {
                    yours.iter().find(|t| t.eq_ignore_ascii_case(want)).cloned().unwrap_or(want.to_string())
                };
                Some(json!({ "foundations": [
                    {"title": named("Linux processes"), "why": format!("{} runs as processes the kernel isolates", s("title"))},
                    {"title": named("Union filesystems"), "why": "images are layers stacked by a union filesystem"},
                    {"title": named("Networking basics"), "why": "services talk over ports and addresses"},
                ]}))
            }
            "check-ground" => {
                // Three questions on the foundation, the key first; why each.
                let f = s("foundation");
                let items: Vec<Value> = (0..3)
                    .map(|i| {
                        json!({
                            "concepts": [f],
                            "tier": "understand",
                            "stem": ([
                                format!("Which statement about {f} holds?"),
                                format!("A colleague asks what {f} is for. What do you tell them?"),
                                format!("Which of these would someone who knows {f} reject as a myth?"),
                            ][i % 3].clone()),
                            "options": [
                                {"text": format!("The core idea of {f}, stated plainly (case {})", i + 1), "correct": true, "why": format!("That's what {f} is about.")},
                                {"text": format!("A common confusion about {f} (case {})", i + 1), "correct": false, "why": "It mixes up two ideas.", "misconception": "a mix-up", "foundation": "none"},
                                {"text": format!("Something unrelated to {f} (case {})", i + 1), "correct": false, "why": "It's about something else.", "misconception": "off the mark", "foundation": "none"},
                                {"text": format!("The opposite of {f}'s idea (case {})", i + 1), "correct": false, "why": "It reverses the idea.", "misconception": "reversed", "foundation": "none"},
                            ],
                            "source": "",
                            "model_knowledge": true,
                        })
                    })
                    .collect();
                Some(json!({ "items": items }))
            }
            "tutor" => {
                // Grounded in the anchor, so a test can see it was loaded.
                let anchor = &input["anchor"];
                let text = match anchor["kind"].as_str() {
                    Some("card") => format!(
                        "You picked {}. The key is {}: {} You asked: {}",
                        anchor["picked"].as_str().unwrap_or("?"),
                        anchor["key"].as_str().unwrap_or("?"),
                        anchor["key_why"].as_str().unwrap_or(""),
                        s("question"),
                    ),
                    Some("resource") => {
                        let excerpt = &anchor["excerpts"][0];
                        format!(
                            "About {}: you asked {}.{} From it{}: {}",
                            anchor["title"].as_str().unwrap_or("this"),
                            s("question"),
                            input["passage"].as_str().map(|p| format!(" On \"{p}\".")).unwrap_or_default(),
                            excerpt["at"].as_str().map(|at| format!(" at {at}")).unwrap_or_default(),
                            excerpt["text"].as_str().unwrap_or("").chars().take(80).collect::<String>(),
                        )
                    }
                    _ => format!(
                        "About {}: you asked {}. From {}: {}",
                        anchor["title"].as_str().unwrap_or("this"),
                        s("question"),
                        anchor["resources"][0]["title"].as_str().unwrap_or("my own knowledge"),
                        anchor["resources"][0]["excerpts"][0]["text"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(80)
                            .collect::<String>(),
                    ),
                };
                Some(json!({ "text": text }))
            }
            "suggest-sources" => {
                // Two real pages and a captioned video, around a dead link,
                // an app shell and a video without captions: kizn should
                // offer the three it can read.
                let concept = s("concept");
                let slug: String = concept
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                    .collect();
                // A first round where nothing can be used (for the concept
                // about registries): kizn should ask again.
                let tried = input["tried"].as_array().is_some_and(|t| !t.is_empty());
                if !tried && concept.contains("registry") {
                    return Some(json!({ "sources": [
                        {"url": "https://old.example.org/web/missing", "title": "Archived notes", "why": "Background reading."},
                        {"url": "https://learn.example.com/web/shell", "title": "Interactive course", "why": "A hands-on walkthrough."},
                    ]}));
                }
                Some(json!({ "sources": [
                    {"url": format!("https://docs.example.org/web/article/{slug}"), "title": format!("{concept}: the guide"), "why": "The official explanation, with a worked example."},
                    {"url": "https://old.example.org/web/missing", "title": "Archived notes", "why": "Background reading."},
                    {"url": "https://learn.example.com/web/shell", "title": "Interactive course", "why": "A hands-on walkthrough."},
                    {"url": format!("https://www.youtube.com/watch?v=nocaptions-{slug}"), "title": "A talk without captions", "why": "A conference talk."},
                    {"url": format!("https://www.youtube.com/watch?v=captioned-{slug}"), "title": format!("{concept}, shown"), "why": "A short demo on a real system."},
                    {"url": format!("https://blog.example.net/web/article/{slug}"), "title": format!("Understanding {concept}"), "why": "A plain-language take on the common mix-ups."},
                ]}))
            }
            "blind-check" => {
                let stem = s("stem");
                let options: Vec<String> = input["options"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|o| o.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let key = self.keys.lock().unwrap().get(&stem).cloned();
                // Options arrive as "A. text".
                let letter_of = |want: &str| {
                    options
                        .iter()
                        .find(|o| o.get(3..) == Some(want))
                        .and_then(|o| o.chars().next())
                        .map(|c| c.to_string())
                };
                let right = key.as_deref().and_then(letter_of).unwrap_or_else(|| "A".into());
                let pick = if b == Behaviour::CheckerDisagrees {
                    options
                        .iter()
                        .filter_map(|o| o.chars().next())
                        .map(|c| c.to_string())
                        .find(|l| *l != right)
                        .unwrap_or(right)
                } else {
                    right
                };
                Some(json!({ "pick": pick, "also_defensible": [] }))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Client, Endpoint, Protocol, Structured, probe};

    async fn check(server: &FakeServer, protocol: Protocol, base_url: String) -> crate::Check {
        let ep = Endpoint { protocol, base_url, model: "m".into(), headers: vec![], key: Some("k".into()) };
        probe(&Client::new(ep, server.client())).await
    }

    #[tokio::test]
    async fn each_behaviour_reads_as_it_should() {
        let s = FakeServer::start(0).await.unwrap();
        for protocol in [Protocol::OpenAi, Protocol::Anthropic] {
            let url = |b| if protocol == Protocol::OpenAi { s.openai_url(b) } else { s.anthropic_url(b) };
            let c = check(&s, protocol, url(Behaviour::Works)).await;
            assert_eq!(
                (c.structured, c.streaming, c.summary.as_str()),
                (Structured::Native, true, "works"),
                "{protocol:?}"
            );
            let c = check(&s, protocol, url(Behaviour::PromptedJson)).await;
            assert_eq!((c.structured, c.streaming), (Structured::Prompted, true), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::NoJson)).await;
            assert_eq!((c.structured, c.streaming), (Structured::None, true), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::NoStream)).await;
            assert_eq!((c.structured, c.streaming), (Structured::Native, false), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::Unauthorized)).await;
            assert!(!c.reachable && c.summary.contains("refused the key"), "{protocol:?}: {}", c.summary);
            let c = check(&s, protocol, url(Behaviour::NoCredit)).await;
            assert!(
                c.reachable && c.summary.starts_with("the account can't run requests yet: Your credit balance"),
                "{protocol:?}: {}",
                c.summary
            );
            let c = check(&s, protocol, url(Behaviour::UnknownModel)).await;
            assert!(c.summary.contains("doesn't have the model m"), "{protocol:?}: {}", c.summary);
        }
    }

    #[tokio::test]
    async fn models_are_listed() {
        let s = FakeServer::start(0).await.unwrap();
        let ep = Endpoint {
            protocol: Protocol::OpenAi,
            base_url: s.openai_url(Behaviour::Works),
            model: String::new(),
            headers: vec![],
            key: None,
        };
        let models = Client::new(ep, s.client()).list_models_strict().await.unwrap();
        assert!(models.contains(&"fake-small".to_string()));
    }

    #[tokio::test]
    async fn a_proxied_host_reaches_the_fake() {
        let s = FakeServer::start(0).await.unwrap();
        s.set_host("gpu", Behaviour::NoJson);
        let c = check(&s, Protocol::OpenAi, "http://gpu:11434/v1".into()).await;
        assert_eq!(c.structured, Structured::None);
    }

    #[tokio::test]
    async fn an_https_provider_reaches_the_fake() {
        let s = FakeServer::start(0).await.unwrap();
        let c = check(&s, Protocol::Anthropic, "https://api.anthropic.com".into()).await;
        assert_eq!(c.summary, "works");
    }

    #[tokio::test]
    async fn a_closed_port_is_unreachable() {
        let s = FakeServer::start(0).await.unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let c = check(&s, Protocol::OpenAi, format!("http://127.0.0.1:{port}/v1")).await;
        assert!(!c.reachable);
        assert_eq!(c.summary, format!("nothing answered at http://127.0.0.1:{port}"));
    }
}
