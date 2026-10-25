//! The connection test: can kizn reach the model, get JSON out of it, and
//! stream from it? Each answer is kept in plain words; which roles that
//! unlocks is decided by the app.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Client, JsonMode, ModelError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Structured {
    /// Follows a JSON schema the protocol enforces.
    Native,
    /// Ignores schemas, but answers in JSON when asked; kizn parses it.
    Prompted,
    /// No usable JSON either way.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub reachable: bool,
    pub structured: Structured,
    pub streaming: bool,
    /// One line: "works", "works; no schema-constrained output, kizn will
    /// parse instead", "nothing answered at …".
    pub summary: String,
    /// What failed, one line each.
    pub problems: Vec<String>,
}

const SYSTEM: &str = "You are being tested by kizn, a learning app, to see whether you can return structured output.";
const ASK: &str = "Reply with ok set to true and word set to \"kizn\".";

fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {"ok": {"type": "boolean"}, "word": {"type": "string"}},
        "required": ["ok", "word"],
        "additionalProperties": false
    })
}

fn fits(v: &serde_json::Value) -> bool {
    v.get("ok").is_some_and(|o| o.is_boolean()) && v.get("word").is_some_and(|w| w.is_string())
}

pub async fn probe(client: &Client) -> Check {
    let mut problems = vec![];

    // 1. Reachable, and the key accepted. (A missing model shows up as the
    //    completion's own 404 below: model lists include aliases and
    //    gateways list partially, so they're not checked against.)
    if let Err(e) = client.list_models().await {
        return Check {
            reachable: false,
            structured: Structured::None,
            streaming: false,
            summary: e.to_string(),
            problems: vec![e.to_string()],
        };
    }

    // 2. Structured output: the protocol's constraint first, then asking.
    let structured = match client.complete_json(SYSTEM, ASK, &schema(), JsonMode::Native).await {
        Ok((v, _)) if fits(&v) => Structured::Native,
        Ok(_) | Err(ModelError::SchemaUnsupported) | Err(ModelError::BadReply(_)) => {
            match client.complete_json(SYSTEM, ASK, &schema(), JsonMode::Prompted).await {
                Ok((v, _)) if fits(&v) => Structured::Prompted,
                Ok(_) => {
                    problems.push("its JSON didn't have the fields kizn asked for".into());
                    Structured::None
                }
                Err(e) => {
                    problems.push(format!("no structured output: {e}"));
                    Structured::None
                }
            }
        }
        Err(e) => {
            // Reachable but the model call itself failed (key, model, status).
            return Check {
                reachable: true,
                structured: Structured::None,
                streaming: false,
                summary: e.to_string(),
                problems: vec![e.to_string()],
            };
        }
    };

    // 3. Streaming: the first delta is enough.
    let streaming = match client.stream_text(SYSTEM, "Say: kizn works.").await {
        Ok(stream) => {
            let mut stream = Box::pin(stream);
            match stream.next().await {
                Some(Ok(_)) => true,
                Some(Err(e)) => {
                    problems.push(format!("streaming failed: {e}"));
                    false
                }
                None => {
                    problems.push("streaming returned nothing".into());
                    false
                }
            }
        }
        Err(e) => {
            problems.push(format!("streaming failed: {e}"));
            false
        }
    };

    let summary = match (structured, streaming) {
        (Structured::Native, true) => "works".to_string(),
        (Structured::Prompted, true) => "works; no schema-constrained output, kizn will parse instead".to_string(),
        (Structured::None, true) => "answers, but can't return structured output: it can tutor, not author items".into(),
        (Structured::Native | Structured::Prompted, false) => {
            "can return structured output, but can't stream: it can author items, not tutor".into()
        }
        (Structured::None, false) => "answers, but can't return structured output or stream".into(),
    };
    Check { reachable: true, structured, streaming, summary, problems }
}
