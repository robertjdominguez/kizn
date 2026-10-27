//! Flags from answers (PRD §6.8): misses whose picked options point at the
//! same foundation. Derived each time from the answers and the questions'
//! foundation tags; only what the learner does with a flag is stored.

use std::collections::HashMap;

use chrono::Duration;
use kizn_core::bank::ItemOption;
use kizn_core::parse_sql_time;
use kizn_store::SuggestionRow;

use crate::command::FlagView;
use crate::foundations::matching;
use crate::{AppError, Ctx};

/// Misses on different questions, within this many days, that raise a flag.
pub const MISSES: usize = 2;
pub const WITHIN_DAYS: i64 = 14;

impl Ctx {
    /// A topic's flags: foundations its recent misses point at.
    pub(crate) fn flags(&self, topic_id: &str) -> Result<Vec<FlagView>, AppError> {
        let topic = self.store.topic(topic_id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?;
        let now = self.deps.clock.now();
        // Foundation title (lowercased) → its title, the topic it names, the questions missed.
        let mut pointed: HashMap<String, (String, Option<String>, Vec<String>)> = HashMap::new();
        for a in self.store.attempts(Some(topic_id))? {
            if a.correct || a.disputed_at.is_some() {
                continue;
            }
            if parse_sql_time(&a.at).is_none_or(|at| now - at > Duration::days(WITHIN_DAYS)) {
                continue;
            }
            let Some(item) = self.store.item(&a.item_id)? else { continue };
            let opts: Vec<ItemOption> =
                serde_json::from_str(item.options_json.as_deref().unwrap_or("[]")).unwrap_or_default();
            let Some(f) = opts.get(a.chosen).and_then(|o| o.foundation.clone()) else { continue };
            let e = pointed.entry(f.title.to_lowercase()).or_insert((f.title.clone(), f.topic_id.clone(), vec![]));
            if !e.2.contains(&a.item_id) {
                e.2.push(a.item_id);
            }
        }
        let topics = self.store.all_topics()?;
        let links = self.store.links()?;
        let handled: Vec<SuggestionRow> = self.store.suggestions(Some(topic_id))?;
        let mut flags: Vec<FlagView> = vec![];
        for (key, (title, tagged, items)) in pointed {
            if items.len() < MISSES || key == topic.title.to_lowercase() {
                continue;
            }
            // Already answered (not now, added, linked) or already on the map.
            let existing = tagged
                .as_deref()
                .and_then(|id| topics.iter().find(|t| t.id == id))
                .or_else(|| matching(&topics, &title, topic_id));
            if handled.iter().any(|s| {
                s.source == "flag" && s.title.to_lowercase() == key && s.status != "open"
                    || s.title.to_lowercase() == key && matches!(s.status.as_str(), "later" | "kept" | "started")
            }) {
                continue;
            }
            let linked = existing.is_some_and(|t| links.iter().any(|l| l.upper_id == topic_id && l.lower_id == t.id));
            let kind = match (existing, linked) {
                (Some(_), true) => "review",
                (Some(_), false) => "link",
                (None, _) => "new",
            };
            flags.push(FlagView {
                for_topic: topic_id.into(),
                for_title: topic.title.clone(),
                title: existing.map(|t| t.title.clone()).unwrap_or(title),
                topic_id: existing.map(|t| t.id.clone()),
                kind: kind.into(),
                misses: items.len() as i64,
            });
        }
        flags.sort_by(|a, b| b.misses.cmp(&a.misses).then(a.title.cmp(&b.title)));
        Ok(flags)
    }

    /// Flags across every confirmed topic, for Today.
    pub(crate) fn all_flags(&self) -> Result<Vec<FlagView>, AppError> {
        let mut out = vec![];
        for t in self.store.all_topics()?.into_iter().filter(|t| t.status == "confirmed") {
            out.extend(self.flags(&t.id)?);
        }
        Ok(out)
    }

    /// What the learner does with a flag: "add" (a suggestion on the map,
    /// under the topic), "link" (the topic they have becomes a foundation)
    /// or "dismiss" ("not now", or "got it" for a review).
    pub(crate) fn resolve_flag(&self, for_topic: &str, title: &str, action: &str) -> Result<(), AppError> {
        let flag = self
            .flags(for_topic)?
            .into_iter()
            .find(|f| f.title.eq_ignore_ascii_case(title))
            .ok_or_else(|| AppError::Refused("that flag isn't up any more".into()))?;
        let status = match action {
            "add" => "later",
            "link" => {
                let lower =
                    flag.topic_id.as_deref().ok_or_else(|| AppError::Refused("there's no topic to link".into()))?;
                self.link_topics(for_topic, lower, "rests_on")?;
                "linked"
            }
            "dismiss" => "dismissed",
            other => return Err(AppError::Refused(format!("there's no way to answer a flag called {other:?}"))),
        };
        self.store.insert_suggestion(&SuggestionRow {
            id: self.id(),
            for_topic: for_topic.into(),
            title: flag.title.clone(),
            why: Some(format!("{} of your misses pointed at it", flag.misses)),
            source: "flag".into(),
            status: status.into(),
            topic_id: flag.topic_id.clone(),
            created_at: self.now(),
        })?;
        self.changed(for_topic);
        Ok(())
    }
}
