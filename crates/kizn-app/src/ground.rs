//! Checking the ground: after a topic is confirmed, a few questions on each
//! foundation kept for it that isn't a topic yet. Diagnostic only (never the
//! topic's concepts or recalls): each foundation comes out "seems solid" or
//! "might be shaky", and the learner decides what to do with a shaky one.

use kizn_core::Tier;
use kizn_core::bank::{CheckContext, Concept, ItemKind, ItemOption, check_item};
use serde_json::{Value, json};

use crate::command::{GroundFoundation, GroundQuestion, GroundView, OptionFeedback, Role};
use crate::{AppError, Ctx};

pub const QUESTIONS: usize = 3;

const CHECK_GROUND: &str = "kizn-task: check-ground
Write multiple-choice questions that check whether an adult learner already has a foundation (a more basic topic) before they start a topic that rests on it. Ask about the central ideas someone who has the foundation would know, not trivia.
- Three to five options, exactly one correct, the others plausible mistakes; every option has \"why\", every wrong option a \"misconception\".
- Keep each stem under 1,200 characters and each option under 250.
Reply with JSON only: {\"items\": [{\"concepts\": [\"<the foundation>\"], \"tier\": \"understand\", \"stem\": \"...\", \"options\": [{\"text\": \"...\", \"correct\": true, \"why\": \"...\"}, {\"text\": \"...\", \"correct\": false, \"why\": \"...\", \"misconception\": \"...\", \"foundation\": \"none\"}], \"source\": \"\", \"model_knowledge\": true}]}";

fn items_schema() -> Value {
    json!({"type": "object", "properties": {"items": {"type": "array", "items": {"type": "object"}}}, "required": ["items"]})
}

/// The order a check question's options are shown in: stable for the
/// question, so a reload shows the same order.
fn order(id: &str, n: usize) -> Vec<usize> {
    let shift = id.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % n.max(1);
    (0..n).map(|i| (i + shift) % n).collect()
}

/// "solid" once answered with at least two in three right; "shaky" otherwise.
fn result(answered: usize, correct: usize, total: usize) -> Option<&'static str> {
    (total > 0 && answered == total).then(|| if correct * 3 >= total * 2 { "solid" } else { "shaky" })
}

impl Ctx {
    /// Foundations to check for a topic: kept, and not a topic yet.
    fn to_check(&self, topic_id: &str) -> Result<Vec<kizn_store::SuggestionRow>, AppError> {
        Ok(self
            .store
            .suggestions(Some(topic_id))?
            .into_iter()
            .filter(|s| s.topic_id.is_none() && matches!(s.status.as_str(), "kept" | "later" | "dismissed" | "started"))
            .filter(|s| s.source == "draft")
            .collect())
    }

    /// Writes the check questions in the background (at confirm).
    pub(crate) fn start_ground(&self, topic_id: &str) {
        if !self.grounding.lock().unwrap().insert(topic_id.to_string()) {
            return;
        }
        let ctx = self.clone();
        let id = topic_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = ctx.write_ground(&id).await {
                eprintln!("kizn: checking the ground: {e}");
            }
            ctx.grounding.lock().unwrap().remove(&id);
            ctx.changed(&id);
        });
    }

    async fn write_ground(&self, topic_id: &str) -> Result<(), AppError> {
        let topic = self.store.topic(topic_id)?.ok_or_else(|| AppError::Refused("no topic".into()))?;
        let author = self.role_model(Role::Author)?;
        for s in self.to_check(topic_id)?.into_iter().filter(|s| s.status == "kept") {
            if !self.store.ground_items(&s.id)?.is_empty() {
                continue;
            }
            let ask = format!(
                "Write {QUESTIONS} questions to check the foundation \"{}\".\n\n<input>\n{}\n</input>",
                s.title,
                json!({ "foundation": s.title, "why": s.why, "topic": topic.title, "goal": topic.goal, "count": QUESTIONS })
            );
            let reply = match author.client.complete_json(CHECK_GROUND, &ask, &items_schema(), author.mode).await {
                Ok((r, _)) => r,
                Err(e) => {
                    eprintln!("kizn: check questions for {}: {e}", s.title);
                    continue;
                }
            };
            let concept = [Concept { id: &s.id, name: &s.title }];
            let mut existing: Vec<(String, Vec<String>)> = vec![];
            for draft in reply["items"].as_array().cloned().unwrap_or_default().iter().take(QUESTIONS) {
                let ctx = CheckContext {
                    kind: ItemKind::Recall,
                    target: Tier::Understand,
                    concepts: &concept,
                    slot_concept_id: Some(&s.id),
                    existing: &existing,
                    topics: &[],
                    topic_id: &topic.id,
                };
                let Ok(item) = check_item(draft, &ctx) else { continue };
                let Some(options) = item.options else { continue };
                self.store.insert_ground_item(
                    &self.id(),
                    &s.id,
                    &item.stem,
                    &serde_json::to_string(&options).expect("options serialise"),
                    &self.now(),
                )?;
                existing.push((item.stem, vec![s.id.clone()]));
            }
        }
        Ok(())
    }

    pub(crate) fn ground_view(&self, topic_id: &str) -> Result<GroundView, AppError> {
        let topic = self.store.topic(topic_id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?;
        let mut foundations = vec![];
        for s in self.to_check(topic_id)? {
            let items = self.store.ground_items(&s.id)?;
            let questions: Vec<GroundQuestion> = items
                .iter()
                .map(|g| {
                    let opts: Vec<ItemOption> = serde_json::from_str(&g.options_json).unwrap_or_default();
                    let shown = order(&g.id, opts.len());
                    GroundQuestion {
                        id: g.id.clone(),
                        stem: g.stem.clone(),
                        options: shown.iter().map(|&i| opts[i].text.clone()).collect(),
                        order: shown.iter().map(|&i| i as u32).collect(),
                        answered: g
                            .answer
                            .map(|(chosen, correct)| {
                                (shown.iter().position(|&i| i == chosen).unwrap_or(0) as u32, correct)
                            })
                            .map(|(p, _)| p),
                        correct: g.answer.map(|(_, c)| c),
                        key: g
                            .answer
                            .and_then(|_| shown.iter().position(|&i| opts.get(i).is_some_and(|o| o.correct)))
                            .map(|p| p as u32),
                        feedback: if g.answer.is_some() {
                            shown
                                .iter()
                                .map(|&i| OptionFeedback {
                                    text: opts[i].text.clone(),
                                    correct: opts[i].correct,
                                    why: opts[i].why.clone(),
                                    misconception: opts[i].misconception.clone(),
                                })
                                .collect()
                        } else {
                            vec![]
                        },
                    }
                })
                .collect();
            let answered = items.iter().filter(|g| g.answer.is_some()).count();
            let correct = items.iter().filter(|g| g.answer.is_some_and(|(_, c)| c)).count();
            foundations.push(GroundFoundation {
                suggestion_id: s.id,
                title: s.title,
                why: s.why,
                status: s.status,
                result: result(answered, correct, items.len()).map(str::to_string),
                questions,
            });
        }
        Ok(GroundView {
            topic_id: topic.id,
            topic_title: topic.title,
            writing: self.grounding.lock().unwrap().contains(topic_id),
            foundations,
        })
    }

    /// Answers a check question (once); returns the check as it now stands.
    pub(crate) fn answer_ground(
        &self,
        item_id: &str,
        order_shown: &[u32],
        chosen: u32,
    ) -> Result<GroundView, AppError> {
        let s = self
            .store
            .suggestions(None)?
            .into_iter()
            .find(|s| self.store.ground_items(&s.id).is_ok_and(|items| items.iter().any(|g| g.id == item_id)))
            .ok_or_else(|| AppError::Refused("there's no such question".into()))?;
        let g = self.store.ground_items(&s.id)?.into_iter().find(|g| g.id == item_id).expect("found above");
        let opts: Vec<ItemOption> = serde_json::from_str(&g.options_json).unwrap_or_default();
        let mut sorted = order_shown.to_vec();
        sorted.sort_unstable();
        if sorted != (0..opts.len() as u32).collect::<Vec<_>>() || chosen as usize >= opts.len() {
            return Err(AppError::Refused("that answer doesn't match the question's options".into()));
        }
        let picked = order_shown[chosen as usize] as usize;
        self.store.answer_ground(item_id, picked, opts[picked].correct, &self.now())?;
        self.changed(&s.for_topic);
        self.ground_view(&s.for_topic)
    }

    /// "Later" (stays a suggestion on the map) or "I've got this" (dismissed).
    pub(crate) fn set_aside_foundation(&self, suggestion_id: &str, dismiss: bool) -> Result<String, AppError> {
        let s = self
            .store
            .suggestions(None)?
            .into_iter()
            .find(|s| s.id == suggestion_id)
            .ok_or_else(|| AppError::Refused("there's no such suggestion".into()))?;
        self.store.set_suggestion(&s.id, if dismiss { "dismissed" } else { "later" }, None)?;
        self.changed(&s.for_topic);
        Ok(s.for_topic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_in_three_seems_solid() {
        assert_eq!(result(3, 2, 3), Some("solid"));
        assert_eq!(result(3, 1, 3), Some("shaky"));
        assert_eq!(result(2, 2, 3), None);
        let o = order("01ABC", 4);
        let mut s = o.clone();
        s.sort_unstable();
        assert_eq!((s, order("01ABC", 4)), (vec![0, 1, 2, 3], o));
    }
}
