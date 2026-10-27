//! Foundations proposed for a topic: from its draft (and, in later slices,
//! from checking the ground and from misses). A proposal that matches a topic
//! the learner has is offered as a link to it; the learner keeps or drops
//! each, one step down at a time.

use kizn_store::{SuggestionRow, TopicRow};
use serde_json::{Value, json};

use crate::command::SuggestionView;
use crate::topics::RoleModel;
use crate::{AppError, Ctx};

pub const MAX_FOUNDATIONS: usize = 3;

const DRAFT_FOUNDATIONS: &str = "kizn-task: draft-foundations
Name the more basic topics a learner should already have before this topic, its foundations: 2 or 3, one step down only (what this topic rests on directly, not the foundations of those). Each is a topic someone could learn on its own (\"Linux processes\", \"Supply and demand\", \"Reading music notation\"), not a concept of this topic.
- If one of the learner's existing topics (listed) is a foundation, name it exactly as listed.
- \"why\": one line on how this topic rests on it.
Reply with JSON only: {\"foundations\": [{\"title\": \"...\", \"why\": \"...\"}]}";

fn foundations_schema() -> Value {
    json!({"type": "object", "properties": {"foundations": {"type": "array", "items": {"type": "object"}}}, "required": ["foundations"]})
}

/// A topic the learner has whose title matches, ignoring case and spacing.
pub(crate) fn matching<'a>(topics: &'a [TopicRow], title: &str, not: &str) -> Option<&'a TopicRow> {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let want = norm(title);
    topics.iter().find(|t| t.id != not && norm(&t.title) == want)
}

impl Ctx {
    /// Asks the author for a topic's foundations, once per draft. A failure
    /// here never fails the draft: the topic just has none proposed.
    pub(crate) async fn draft_foundations(&self, topic: &TopicRow, author: &RoleModel) -> Result<(), AppError> {
        if self.store.suggestions(Some(&topic.id))?.iter().any(|s| s.source == "draft") {
            return Ok(());
        }
        let topics: Vec<TopicRow> = self.store.all_topics()?.into_iter().filter(|t| t.id != topic.id).collect();
        let outcomes: Vec<String> = self.store.outcomes(&topic.id)?.into_iter().map(|o| o.statement).collect();
        let ask = format!(
            "<input>\n{}\n</input>",
            json!({ "title": topic.title, "goal": topic.goal, "outcomes": outcomes,
                    "your_topics": topics.iter().map(|t| t.title.as_str()).collect::<Vec<_>>() })
        );
        let reply = match author.client.complete_json(DRAFT_FOUNDATIONS, &ask, &foundations_schema(), author.mode).await
        {
            Ok((r, _)) => r,
            Err(e) => {
                eprintln!("kizn: drafting foundations: {e}");
                return Ok(());
            }
        };
        let mut seen: Vec<String> = vec![];
        for f in reply["foundations"].as_array().cloned().unwrap_or_default().iter().take(MAX_FOUNDATIONS) {
            let Some(title) = f["title"].as_str().map(str::trim).filter(|t| !t.is_empty()) else { continue };
            if seen.contains(&title.to_lowercase()) || title.eq_ignore_ascii_case(&topic.title) {
                continue;
            }
            seen.push(title.to_lowercase());
            let existing = matching(&topics, title, &topic.id);
            self.store.insert_suggestion(&SuggestionRow {
                id: self.id(),
                for_topic: topic.id.clone(),
                title: existing.map(|t| t.title.clone()).unwrap_or_else(|| title.to_string()),
                why: f["why"].as_str().map(str::to_string),
                source: "draft".into(),
                // Kept unless the learner drops it in Shape.
                status: "kept".into(),
                topic_id: existing.map(|t| t.id.clone()),
                created_at: self.now(),
            })?;
        }
        Ok(())
    }

    /// Keep or drop a proposed foundation (while shaping a draft).
    pub(crate) fn keep_foundation(&self, suggestion_id: &str, keep: bool) -> Result<String, AppError> {
        let s = self
            .store
            .suggestions(None)?
            .into_iter()
            .find(|s| s.id == suggestion_id)
            .ok_or_else(|| AppError::Refused("there's no such suggestion".into()))?;
        self.store.set_suggestion(&s.id, if keep { "kept" } else { "dropped" }, None)?;
        self.changed(&s.for_topic);
        Ok(s.for_topic)
    }

    /// At confirm: kept foundations that are topics the learner has become
    /// links (unless one would make a loop). The rest wait to be checked.
    pub(crate) fn link_kept_foundations(&self, topic_id: &str) -> Result<(), AppError> {
        for s in self.store.suggestions(Some(topic_id))? {
            if s.status == "kept"
                && let Some(lower) = &s.topic_id
            {
                match self.link_topics(topic_id, lower, "rests_on") {
                    Ok(()) => self.store.set_suggestion(&s.id, "linked", None)?,
                    Err(e) => eprintln!("kizn: not linking {}: {e}", s.title),
                }
            }
        }
        Ok(())
    }

    /// A topic's proposed foundations, for its views.
    pub(crate) fn suggestion_views(&self, topic_id: &str) -> Result<Vec<SuggestionView>, AppError> {
        Ok(self
            .store
            .suggestions(Some(topic_id))?
            .into_iter()
            .map(|s| SuggestionView {
                topic_title: s.topic_id.as_deref().and_then(|t| self.store.topic(t).ok().flatten()).map(|t| t.title),
                id: s.id,
                title: s.title,
                why: s.why,
                source: s.source,
                status: s.status,
                topic_id: s.topic_id,
            })
            .collect())
    }
}
