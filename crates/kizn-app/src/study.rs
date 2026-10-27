//! Study, thin: for a concept the learner missed, the tutor suggests pages to
//! read, and kizn fetches each one to check it's real, readable and on the
//! concept before offering it (kizn_core::source). The model suggests; code
//! decides what's offered. Studying never changes a concept's state.

use std::time::Duration;

use kizn_core::progress::ConceptState;
use kizn_core::source::verify;
use kizn_store::SourceRow;
use serde_json::{Value, json};

use crate::command::{ConceptPageView, Role, SourceView};
use crate::practice::days_until;
use crate::{AppError, Ctx};

/// Verified sources offered per concept.
pub const MAX_VERIFIED: usize = 2;
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;

const SUGGEST_SOURCES: &str = "kizn-task: suggest-sources
You suggest web pages an adult learner can read to learn one concept. kizn fetches every page you suggest and only offers the ones that load as readable text about the concept, so suggest pages you're confident exist at that exact URL.
- Prefer official documentation, standards, and well-known references over blogs; no videos, no paywalled or login-only pages, no PDFs.
- Up to 4 pages, each a different URL than those listed as tried.
- \"why\": one line on what the learner gets from it.
Reply with JSON only: {\"sources\": [{\"url\": \"https://...\", \"title\": \"...\", \"why\": \"...\"}]}";

fn sources_schema() -> Value {
    json!({"type": "object", "properties": {"sources": {"type": "array", "items": {"type": "object"}}}, "required": ["sources"]})
}

fn host(url: &str) -> String {
    kizn_model::reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_default()
}

impl Ctx {
    pub(crate) fn concept_page(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        let concept =
            self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("there's no such concept".into()))?;
        let outcome = self
            .store
            .outcome(&concept.outcome_id)?
            .ok_or_else(|| AppError::Refused("that concept's outcome is gone".into()))?;
        let topic = self
            .store
            .topic(&outcome.topic_id)?
            .ok_or_else(|| AppError::Refused("that concept's topic is gone".into()))?;
        let rows = self.store.sources(concept_id)?;
        let verified: Vec<SourceView> = rows
            .iter()
            .filter(|s| s.status == "verified")
            .map(|s| SourceView {
                url: s.url.clone(),
                title: s.title.clone(),
                why: s.why.clone(),
                host: host(&s.url),
                words: s.words.unwrap_or(0),
            })
            .collect();
        let finding = if self.sourcing.lock().unwrap().contains(concept_id) {
            "finding"
        } else if !verified.is_empty() {
            "found"
        } else if rows.is_empty() {
            "not_started"
        } else {
            "none"
        };
        let standing = self.standing_of_concept(&topic.id, concept_id)?;
        let now = self.deps.clock.now();
        Ok(ConceptPageView {
            concept_id: concept.id,
            statement: concept.statement,
            topic_id: topic.id,
            topic_title: topic.title,
            outcome_ord: outcome.ord,
            state: standing.state.name().into(),
            next_due_in_days: standing.due.map(|d| days_until(d, now)),
            studied: self.store.studied_at(concept_id)?.is_some(),
            sources: verified,
            finding: finding.into(),
            checked: rows.iter().filter(|s| !s.url.is_empty()).count() as i64,
        })
    }

    /// The page, starting the search for sources if there's nothing yet.
    pub(crate) fn open_concept(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        let page = self.concept_page(concept_id)?;
        if page.finding == "not_started" {
            self.start_finding(concept_id);
            return self.concept_page(concept_id);
        }
        Ok(page)
    }

    pub(crate) fn find_again(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        self.start_finding(concept_id);
        self.concept_page(concept_id)
    }

    pub(crate) fn mark_studied(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        let page = self.concept_page(concept_id)?;
        self.store.mark_studied(concept_id, &self.now())?;
        self.changed(&page.topic_id);
        self.concept_page(concept_id)
    }

    /// Looks for sources in the background, once at a time per concept.
    pub(crate) fn start_finding(&self, concept_id: &str) {
        if !self.sourcing.lock().unwrap().insert(concept_id.to_string()) {
            return;
        }
        let ctx = self.clone();
        let id = concept_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = ctx.find_sources(&id).await {
                eprintln!("kizn: finding sources: {e}");
            }
            ctx.sourcing.lock().unwrap().remove(&id);
            if let Ok(Some(c)) = ctx.store.concept(&id)
                && let Ok(Some(o)) = ctx.store.outcome(&c.outcome_id)
            {
                ctx.changed(&o.topic_id);
            }
        });
    }

    /// For a concept just missed with nothing to read yet: start looking, so
    /// the sources are there by the time the summary is.
    pub(crate) fn find_if_missed(&self, concept_id: &str, state: ConceptState) {
        if state == ConceptState::Seen && self.store.sources(concept_id).is_ok_and(|s| s.is_empty()) {
            self.start_finding(concept_id);
        }
    }

    async fn find_sources(&self, concept_id: &str) -> Result<(), AppError> {
        let concept = self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("no such concept".into()))?;
        let outcome = self.store.outcome(&concept.outcome_id)?.ok_or_else(|| AppError::Refused("no outcome".into()))?;
        let topic = self.store.topic(&outcome.topic_id)?.ok_or_else(|| AppError::Refused("no topic".into()))?;
        let tried = self.store.sources(concept_id)?;
        let mut verified = tried.iter().filter(|s| s.status == "verified").count();
        if verified >= MAX_VERIFIED {
            return Ok(());
        }
        let tutor = self.role_model(Role::Tutor).or_else(|_| self.role_model(Role::Author))?;
        let ask = format!(
            "Suggest up to 4 pages to learn this concept: \"{}\".\n\n<input>\n{}\n</input>",
            concept.statement,
            json!({
                "topic": topic.title, "goal": topic.goal, "outcome": outcome.statement, "concept": concept.statement,
                "tried": tried.iter().map(|s| s.url.as_str()).filter(|u| !u.is_empty()).collect::<Vec<_>>(),
            })
        );
        let reply = match tutor.client.complete_json(SUGGEST_SOURCES, &ask, &sources_schema(), tutor.mode).await {
            Ok((reply, _)) => reply,
            Err(e) => {
                // Recorded so the page can say it couldn't look, not "nothing found".
                self.store.insert_source(&SourceRow {
                    id: self.id(),
                    concept_id: concept_id.into(),
                    url: String::new(),
                    title: String::new(),
                    why: None,
                    status: "failed".into(),
                    reason: Some(format!("the tutor couldn't suggest any: {e}")),
                    words: None,
                    checked_at: self.now(),
                })?;
                return Ok(());
            }
        };
        let candidates = reply["sources"].as_array().cloned().unwrap_or_default();
        for c in candidates.iter().take(4) {
            if verified >= MAX_VERIFIED {
                break;
            }
            let Some(url) =
                c["url"].as_str().map(str::trim).filter(|u| u.starts_with("https://") || u.starts_with("http://"))
            else {
                continue;
            };
            if tried.iter().any(|t| t.url == url) {
                continue;
            }
            let fetched = self.deps.http.fetch_page(url, FETCH_TIMEOUT, MAX_PAGE_BYTES).await;
            let (status, reason, words, page_title) = match fetched {
                Err(e) => ("failed", Some(format!("unreachable: {e}")), None, None),
                Ok(page) => match verify(page.status, &page.content_type, &page.body, &concept.statement) {
                    Ok(v) => ("verified", None, Some(v.words as i64), v.title),
                    Err(r) => ("rejected", Some(r.to_string()), None, None),
                },
            };
            let title = c["title"].as_str().map(str::to_string).filter(|t| !t.trim().is_empty()).or(page_title);
            self.store.insert_source(&SourceRow {
                id: self.id(),
                concept_id: concept_id.into(),
                url: url.into(),
                title: title.unwrap_or_else(|| host(url)),
                why: c["why"].as_str().map(str::to_string),
                status: status.into(),
                reason,
                words,
                checked_at: self.now(),
            })?;
            if status == "verified" {
                verified += 1;
            }
        }
        Ok(())
    }
}
