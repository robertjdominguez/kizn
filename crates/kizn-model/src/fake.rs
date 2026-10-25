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
}

impl Behaviour {
    pub const ALL: [Behaviour; 6] = [
        Behaviour::Works,
        Behaviour::PromptedJson,
        Behaviour::NoJson,
        Behaviour::NoStream,
        Behaviour::Unauthorized,
        Behaviour::UnknownModel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Behaviour::Works => "works",
            Behaviour::PromptedJson => "prompted-json",
            Behaviour::NoJson => "no-json",
            Behaviour::NoStream => "no-stream",
            Behaviour::Unauthorized => "unauthorized",
            Behaviour::UnknownModel => "unknown-model",
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
}

impl FakeServer {
    /// Starts on 127.0.0.1 (port 0 picks a free one).
    pub async fn start(port: u16) -> std::io::Result<FakeServer> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        let mut server = FakeServer::default();
        server.addr = Some(listener.local_addr()?);
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
    match (method, rest.as_str()) {
        (Method::GET, "/v1/models") => axum::Json(json!({
            "object": "list",
            "data": [{"id": "fake-small"}, {"id": "fake-large"}, {"id": "qwen3:32b"}, {"id": "claude-sonnet-5"}]
        }))
        .into_response(),
        (Method::POST, "/v1/chat/completions") => openai(behaviour, &req),
        (Method::POST, "/v1/messages") => anthropic(behaviour, &req),
        _ => error(StatusCode::NOT_FOUND, "no such route"),
    }
}

const ANSWER: &str = r#"{"ok": true, "word": "kizn"}"#;
const PROSE: &str = "Sure! I'd be happy to help with that. kizn works.";

fn error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(json!({"error": {"message": message}}))).into_response()
}

fn sse(events: Vec<String>) -> Response {
    let body: String = events.into_iter().map(|e| format!("data: {e}\n\n")).collect();
    Response::builder().header(header::CONTENT_TYPE, "text/event-stream").body(Body::from(body)).unwrap()
}

/// Whether the request asks for JSON in its prompt (Prompted mode).
fn asks_for_json(req: &Value) -> bool {
    req.to_string().contains("Reply with only a JSON object")
}

fn openai(b: Behaviour, req: &Value) -> Response {
    if b == Behaviour::UnknownModel {
        return error(StatusCode::NOT_FOUND, "model not found");
    }
    if req["stream"] == true {
        if b == Behaviour::NoStream {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "streaming is broken here");
        }
        let mut events: Vec<String> = ["kizn ", "works."]
            .iter()
            .map(|t| json!({"choices": [{"delta": {"content": t}}]}).to_string())
            .collect();
        events.push("[DONE]".into());
        return sse(events);
    }
    let schema = !req["response_format"].is_null();
    let content = match (b, schema) {
        (Behaviour::PromptedJson, true) => return error(StatusCode::BAD_REQUEST, "response_format is not supported"),
        (Behaviour::NoJson, _) => PROSE.to_string(),
        (_, true) => ANSWER.to_string(),
        (_, false) if asks_for_json(req) => format!("Here you go:\n```json\n{ANSWER}\n```"),
        (_, false) => PROSE.to_string(),
    };
    axum::Json(json!({
        "choices": [{"message": {"role": "assistant", "content": content}}],
        "usage": {"prompt_tokens": 20, "completion_tokens": 8}
    }))
    .into_response()
}

fn anthropic(b: Behaviour, req: &Value) -> Response {
    if b == Behaviour::UnknownModel {
        return error(StatusCode::NOT_FOUND, "model: not found");
    }
    if req["stream"] == true {
        if b == Behaviour::NoStream {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "streaming is broken here");
        }
        let mut events = vec![json!({"type": "message_start"}).to_string()];
        for t in ["kizn ", "works."] {
            events.push(json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": t}}).to_string());
        }
        events.push(json!({"type": "message_stop"}).to_string());
        return sse(events);
    }
    let tools = !req["tools"].is_null();
    let content = match (b, tools) {
        (Behaviour::PromptedJson, true) => return error(StatusCode::BAD_REQUEST, "tools are not supported"),
        (Behaviour::NoJson, _) => json!([{"type": "text", "text": PROSE}]),
        (_, true) => json!([{"type": "tool_use", "name": "reply", "input": {"ok": true, "word": "kizn"}}]),
        (_, false) if asks_for_json(req) => json!([{"type": "text", "text": ANSWER}]),
        (_, false) => json!([{"type": "text", "text": PROSE}]),
    };
    axum::Json(json!({"content": content, "usage": {"input_tokens": 20, "output_tokens": 8}})).into_response()
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
            assert_eq!((c.structured, c.streaming, c.summary.as_str()), (Structured::Native, true, "works"), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::PromptedJson)).await;
            assert_eq!((c.structured, c.streaming), (Structured::Prompted, true), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::NoJson)).await;
            assert_eq!((c.structured, c.streaming), (Structured::None, true), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::NoStream)).await;
            assert_eq!((c.structured, c.streaming), (Structured::Native, false), "{protocol:?}");
            let c = check(&s, protocol, url(Behaviour::Unauthorized)).await;
            assert!(!c.reachable && c.summary.contains("refused the key"), "{protocol:?}: {}", c.summary);
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
