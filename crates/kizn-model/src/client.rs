use std::time::Duration;

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ModelError;
use crate::sse::data_events;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// OpenAI-compatible chat completions: Ollama, llama.cpp, vLLM, LM
    /// Studio, OpenRouter, OpenAI, Gemini's compatibility endpoint.
    #[serde(rename = "openai")]
    OpenAi,
    /// Anthropic Messages.
    Anthropic,
}

impl Protocol {
    pub fn default_base_url(self) -> &'static str {
        match self {
            Protocol::OpenAi => "http://localhost:11434/v1",
            Protocol::Anthropic => "https://api.anthropic.com",
        }
    }
}

/// Where a model lives and how to reach it. The key is the resolved secret,
/// never written anywhere by this crate.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub protocol: Protocol,
    pub base_url: String,
    pub model: String,
    pub headers: Vec<(String, String)>,
    pub key: Option<String>,
}

/// How to get JSON out of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonMode {
    /// The protocol's own constraint: `response_format: json_schema`
    /// (OpenAI) or a forced tool call (Anthropic).
    Native,
    /// Ask for JSON in the prompt and parse leniently.
    Prompted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// How model traffic leaves the app: straight out, or (tests and mock mode)
/// redirected to the fake model server.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
    /// `http://127.0.0.1:PORT`: every non-loopback request goes here
    /// instead, with its original host in `x-kizn-host`, so `https://`
    /// providers can be faked too. Loopback stays direct, so a closed local
    /// port is still closed.
    redirect: Option<String>,
}

/// Carries the original host of a redirected request.
pub const REDIRECT_HOST_HEADER: &str = "x-kizn-host";

impl Http {
    pub fn real() -> Http {
        Http { client: reqwest::Client::new(), redirect: None }
    }

    pub fn redirected(origin: impl Into<String>) -> Http {
        Http { client: reqwest::Client::new(), redirect: Some(origin.into()) }
    }

    fn route(&self, url: String) -> (String, Option<String>) {
        let Some(origin) = &self.redirect else { return (url, None) };
        let Ok(parsed) = reqwest::Url::parse(&url) else { return (url, None) };
        let host = parsed.host_str().unwrap_or("").to_string();
        let loopback = host == "localhost" || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if loopback {
            return (url, None);
        }
        let query = parsed.query().map(|q| format!("?{q}")).unwrap_or_default();
        (format!("{origin}{}{query}", parsed.path()), Some(host))
    }
}

impl Default for Http {
    fn default() -> Self {
        Http::real()
    }
}

pub struct Client {
    endpoint: Endpoint,
    http: Http,
    timeout: Duration,
}

impl Client {
    pub fn new(endpoint: Endpoint, http: Http) -> Client {
        Client { endpoint, http, timeout: Duration::from_secs(20) }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Client {
        self.timeout = timeout;
        self
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn base(&self) -> &str {
        self.endpoint.base_url.trim_end_matches('/')
    }

    fn url(&self, path: &str) -> String {
        match self.endpoint.protocol {
            Protocol::OpenAi => format!("{}/{path}", self.base()),
            Protocol::Anthropic => {
                let base = self.base().trim_end_matches("/v1");
                format!("{base}/v1/{path}")
            }
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let (url, redirected_from) = self.http.route(self.url(path));
        let mut r = self.http.client.request(method, url).timeout(self.timeout);
        if let Some(host) = redirected_from {
            r = r.header(REDIRECT_HOST_HEADER, host);
        }
        match self.endpoint.protocol {
            Protocol::OpenAi => {
                if let Some(k) = &self.endpoint.key {
                    r = r.bearer_auth(k);
                }
            }
            Protocol::Anthropic => {
                r = r.header("anthropic-version", "2023-06-01");
                if let Some(k) = &self.endpoint.key {
                    r = r.header("x-api-key", k);
                }
            }
        }
        for (k, v) in &self.endpoint.headers {
            r = r.header(k, v);
        }
        r
    }

    async fn send(&self, r: reqwest::RequestBuilder, url: &str) -> Result<reqwest::Response, ModelError> {
        let res = r.send().await.map_err(|e| ModelError::from_reqwest(e, url, self.timeout.as_secs()))?;
        let status = res.status().as_u16();
        if status >= 400 {
            let body = res.text().await.unwrap_or_default();
            return Err(ModelError::from_status(status, &body, &self.endpoint.model));
        }
        Ok(res)
    }

    /// The models the endpoint lists, if it lists any. Reaching it at all is
    /// what the connection test's first step needs.
    pub async fn list_models(&self) -> Result<Vec<String>, ModelError> {
        let url = self.url("models");
        let res = self.request(reqwest::Method::GET, "models").send().await;
        let res = res.map_err(|e| ModelError::from_reqwest(e, &url, self.timeout.as_secs()))?;
        let status = res.status().as_u16();
        if status == 401 || status == 403 {
            return Err(ModelError::KeyRefused(status));
        }
        if status >= 400 {
            // Something answered; not every gateway lists models.
            return Ok(vec![]);
        }
        let v: Value = res.json().await.unwrap_or(Value::Null);
        Ok(v.get("data")
            .and_then(|d| d.as_array())
            .map(|a| a.iter().filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_string)).collect())
            .unwrap_or_default())
    }

    /// The models the endpoint lists, failing on any error: for a model
    /// picker, where "couldn't list them" should be said.
    pub async fn list_models_strict(&self) -> Result<Vec<String>, ModelError> {
        let url = self.url("models");
        let res = self.send(self.request(reqwest::Method::GET, "models"), &url).await?;
        let v: Value = res.json().await.map_err(|e| ModelError::BadReply(e.to_string()))?;
        let models: Vec<String> = v
            .get("data")
            .or_else(|| v.get("models"))
            .and_then(|d| d.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|m| m.get("id").or_else(|| m.get("name")).and_then(|i| i.as_str()))
                    .map(|id| id.trim_start_matches("models/").to_string())
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    /// One JSON object matching `schema` (a JSON Schema object).
    pub async fn complete_json(
        &self,
        system: &str,
        user: &str,
        schema: &Value,
        mode: JsonMode,
    ) -> Result<(Value, Usage), ModelError> {
        let system = match mode {
            JsonMode::Native => system.to_string(),
            JsonMode::Prompted => format!(
                "{system}\n\nReply with only a JSON object, no prose and no code fence, matching this JSON Schema:\n{schema}"
            ),
        };
        match self.endpoint.protocol {
            Protocol::OpenAi => {
                let mut body = json!({
                    "model": self.endpoint.model,
                    "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
                    "stream": false,
                });
                if mode == JsonMode::Native {
                    body["response_format"] = json!({"type": "json_schema", "json_schema": {"name": "reply", "schema": schema, "strict": true}});
                }
                let v = self.post_json("chat/completions", &body, mode).await?;
                let content = v.pointer("/choices/0/message/content").and_then(|c| c.as_str()).unwrap_or_default();
                let usage = Usage {
                    prompt_tokens: v.pointer("/usage/prompt_tokens").and_then(|n| n.as_u64()).unwrap_or(0),
                    completion_tokens: v.pointer("/usage/completion_tokens").and_then(|n| n.as_u64()).unwrap_or(0),
                };
                Ok((parse_json_object(content)?, usage))
            }
            Protocol::Anthropic => {
                let mut body = json!({
                    "model": self.endpoint.model,
                    "max_tokens": 2048,
                    "system": system,
                    "messages": [{"role": "user", "content": user}],
                });
                if mode == JsonMode::Native {
                    body["tools"] =
                        json!([{"name": "reply", "description": "Give the reply.", "input_schema": schema}]);
                    body["tool_choice"] = json!({"type": "tool", "name": "reply"});
                }
                let v = self.post_json("messages", &body, mode).await?;
                let usage = Usage {
                    prompt_tokens: v.pointer("/usage/input_tokens").and_then(|n| n.as_u64()).unwrap_or(0),
                    completion_tokens: v.pointer("/usage/output_tokens").and_then(|n| n.as_u64()).unwrap_or(0),
                };
                let blocks = v.get("content").and_then(|c| c.as_array()).cloned().unwrap_or_default();
                if let Some(input) = blocks.iter().find(|b| b["type"] == "tool_use").map(|b| b["input"].clone()) {
                    return Ok((input, usage));
                }
                let text: String = blocks.iter().filter_map(|b| b["text"].as_str()).collect();
                Ok((parse_json_object(&text)?, usage))
            }
        }
    }

    async fn post_json(&self, path: &str, body: &Value, mode: JsonMode) -> Result<Value, ModelError> {
        let url = self.url(path);
        match self.send(self.request(reqwest::Method::POST, path).json(body), &url).await {
            Ok(res) => res.json().await.map_err(|e| ModelError::BadReply(e.to_string())),
            // A schema the endpoint won't take is a 400/422 on the native call.
            Err(ModelError::Status { status: 400 | 422, .. }) if mode == JsonMode::Native => {
                Err(ModelError::SchemaUnsupported)
            }
            Err(e) => Err(e),
        }
    }

    /// Streamed text, delta by delta.
    pub async fn stream_text(
        &self,
        system: &str,
        user: &str,
    ) -> Result<impl Stream<Item = Result<String, ModelError>> + use<>, ModelError> {
        let (path, body) = match self.endpoint.protocol {
            Protocol::OpenAi => (
                "chat/completions",
                json!({
                    "model": self.endpoint.model,
                    "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
                    "stream": true,
                }),
            ),
            Protocol::Anthropic => (
                "messages",
                json!({
                    "model": self.endpoint.model,
                    "max_tokens": 1024,
                    "system": system,
                    "messages": [{"role": "user", "content": user}],
                    "stream": true,
                }),
            ),
        };
        let url = self.url(path);
        let res = self.send(self.request(reqwest::Method::POST, path).json(&body), &url).await?;
        let protocol = self.endpoint.protocol;
        let events = data_events(Box::pin(res.bytes_stream()));
        Ok(events.filter_map(move |ev| async move {
            let data = match ev {
                Ok(d) => d,
                Err(e) => return Some(Err(ModelError::BadReply(format!("the stream broke: {e}")))),
            };
            if data.trim() == "[DONE]" {
                return None;
            }
            let v: Value = serde_json::from_str(&data).ok()?;
            let delta = match protocol {
                Protocol::OpenAi => v.pointer("/choices/0/delta/content").and_then(|c| c.as_str()),
                Protocol::Anthropic => (v["type"] == "content_block_delta")
                    .then(|| v.pointer("/delta/text").and_then(|t| t.as_str()))
                    .flatten(),
            };
            delta.filter(|d| !d.is_empty()).map(|d| Ok(d.to_string()))
        }))
    }
}

/// The first `{...}` in a reply, parsed: lenient about code fences and a
/// stray sentence around it, strict about the object itself.
pub fn parse_json_object(text: &str) -> Result<Value, ModelError> {
    let start = text.find('{');
    let end = text.rfind('}');
    let (Some(s), Some(e)) = (start, end) else {
        return Err(ModelError::BadReply("no JSON object in the reply".into()));
    };
    if e <= s {
        return Err(ModelError::BadReply("no JSON object in the reply".into()));
    }
    let v: Value =
        serde_json::from_str(&text[s..=e]).map_err(|e| ModelError::BadReply(format!("invalid JSON: {e}")))?;
    if !v.is_object() {
        return Err(ModelError::BadReply("the reply isn't a JSON object".into()));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_found_inside_prose_and_fences() {
        let v = parse_json_object("Sure!\n```json\n{\"ok\": true}\n```").unwrap();
        assert_eq!(v["ok"], true);
        assert!(parse_json_object("no json here").is_err());
        assert!(parse_json_object("[1, 2]").is_err());
    }
}
