use std::error::Error as _;

/// What went wrong talking to a model, phrased for the learner.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ModelError {
    #[error("nothing answered at {0}")]
    Unreachable(String),
    #[error("couldn't find the host {0}")]
    HostNotFound(String),
    #[error("no answer within {0} seconds")]
    Timeout(u64),
    #[error("the endpoint refused the key (HTTP {0})")]
    KeyRefused(u16),
    #[error("the endpoint doesn't have the model {0}")]
    UnknownModel(String),
    /// The key works, but the account can't run requests: no credit, quota
    /// used up, billing not set up.
    #[error("the account can't run requests yet: {0}")]
    Account(String),
    #[error("the endpoint doesn't accept a JSON schema")]
    SchemaUnsupported,
    #[error("the endpoint answered HTTP {status}: {detail}")]
    Status { status: u16, detail: String },
    #[error("the reply wasn't what kizn asked for: {0}")]
    BadReply(String),
    #[error("{0}")]
    Config(String),
}

impl ModelError {
    pub(crate) fn from_reqwest(e: reqwest::Error, url: &str, timeout_secs: u64) -> ModelError {
        if e.is_timeout() {
            return ModelError::Timeout(timeout_secs);
        }
        if e.is_connect() {
            let mut chain = String::new();
            let mut src = e.source();
            while let Some(s) = src {
                chain.push_str(&s.to_string().to_lowercase());
                chain.push(' ');
                src = s.source();
            }
            let host = reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string));
            if chain.contains("dns") || chain.contains("lookup") || chain.contains("resolve") {
                return ModelError::HostNotFound(host.unwrap_or_else(|| url.to_string()));
            }
            return ModelError::Unreachable(origin(url));
        }
        ModelError::BadReply(e.to_string())
    }

    /// From an error status: the common ones get their own words.
    pub(crate) fn from_status(status: u16, body: &str, model: &str) -> ModelError {
        let lower = body.to_lowercase();
        let account = ["credit balance", "insufficient_quota", "billing", "quota", "payment"];
        if status == 402 || ((status == 400 || status == 429) && account.iter().any(|w| lower.contains(w))) {
            return ModelError::Account(snippet(body));
        }
        match status {
            401 | 403 => ModelError::KeyRefused(status),
            404 if lower.contains("model") => ModelError::UnknownModel(model.to_string()),
            _ => ModelError::Status { status, detail: snippet(body) },
        }
    }
}

/// `scheme://host:port` of a URL: what "nothing answered at" names.
fn origin(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(u) => format!(
            "{}://{}{}",
            u.scheme(),
            u.host_str().unwrap_or(""),
            u.port().map(|p| format!(":{p}")).unwrap_or_default()
        ),
        Err(_) => url.to_string(),
    }
}

/// The useful part of an error body: its message if it's JSON, else the
/// first line, kept short.
fn snippet(body: &str) -> String {
    let msg = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message").or_else(|| v.get("message")).and_then(|m| m.as_str()).map(str::to_string)
        })
        .unwrap_or_else(|| body.lines().next().unwrap_or("").to_string());
    let msg = msg.trim();
    if msg.chars().count() > 160 {
        format!("{}…", msg.chars().take(160).collect::<String>())
    } else {
        msg.to_string()
    }
}
