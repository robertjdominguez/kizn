//! Writing the bank: closed items per outcome, written by the author model in
//! the background, checked in code, then answered blind by the checker model
//! in a fresh call. An item goes live only if both pass; a refused one is kept
//! with its reason and never shown. Nobody waits on this, so it can be slow
//! and retried.

use std::collections::HashSet;
use std::time::Duration;

use kizn_core::bank::{
    BankedItem, CheckContext, Concept, ItemKind, LETTERS, TopicRef, bank_needs, blind_verdict, check_item, parse_blind,
    shuffled_order,
};
use kizn_core::{Tier, sql_time};
use kizn_model::ModelError;
use kizn_store::{BatchCounts, ItemRow, OutcomeRow};
use serde_json::{Value, json};

use crate::command::Role;
use crate::topics::RoleModel;
use crate::{AppError, Ctx};

/// Items asked for per call: small, so a thinking model rarely runs out.
const ITEMS_PER_CALL: usize = 2;
/// After an account or connection failure, when to try again by itself.
const RETRY_AFTER_MINUTES: i64 = 5;
/// After two batches in a row accept nothing, how long to leave it.
const BACKOFF_HOURS: i64 = 6;

const WRITE_ITEMS: &str = "kizn-task: write-items
You write practice items for one adult learner. They are shown later, one at a time, as cards; nobody is waiting on you. Code checks every item, then another model answers each one blind, so write items with one clearly best answer.

Rules for every scored item (recall, apply, target):
- Ask only about the concepts given, as their statements put them. Nothing outside them.
- Three to five options. Exactly one correct. The others are plausible to someone holding a real misconception, not wordplay or trick wording.
- Every option has \"why\": one line on why it's right, or what's wrong with it.
- Every wrong option has \"misconception\": the mistaken belief it stands for, in one line.
- Every wrong option has \"foundation\": if its misconception is really a gap in a more basic topic (one of the topics listed, or another more basic one), name that topic; otherwise \"none\".
- Options alike in length, grammar and style. No \"all of the above\", \"none of the above\", \"both A and C\". Don't repeat the correct option's words in the stem.
- Scenarios (apply, target) use concrete details; numbers and names are made up and the stem says so.
- \"source\": what you took it from, or \"\" with \"model_knowledge\": true.
- Don't reuse or lightly reword the stems listed as existing.

Reply with JSON only:
{\"items\": [{\"concepts\": [\"<concept>\", ...], \"tier\": \"<bloom tier>\", \"stem\": \"...\", \"options\": [{\"text\": \"...\", \"correct\": true, \"why\": \"...\"}, {\"text\": \"...\", \"correct\": false, \"why\": \"...\", \"misconception\": \"...\", \"foundation\": \"none\"}], \"source\": \"\", \"model_knowledge\": true}]}

An explain item has no options: {\"concepts\": [...], \"stem\": \"a prompt asking them to explain something from the outcome to a colleague or a customer\", \"exemplar\": \"a strong answer, 80 to 200 words\", \"source\": \"\", \"model_knowledge\": true}";

const BLIND_CHECK: &str = "kizn-task: blind-check
Answer a multiple-choice question as an expert would. Then say whether any other option could also be defended as correct by an expert.
Reply with JSON only: {\"pick\": \"<letter>\", \"also_defensible\": [\"<letter>\", ...]}";

fn items_schema() -> Value {
    json!({"type": "object", "properties": {"items": {"type": "array", "items": {"type": "object"}}}, "required": ["items"]})
}

fn blind_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"pick": {"type": "string"}, "also_defensible": {"type": "array", "items": {"type": "string"}}},
        "required": ["pick", "also_defensible"], "additionalProperties": false
    })
}

impl Ctx {
    /// Runs forever: writes the next outcome that needs items, or waits to
    /// be woken (a confirm, a retry) or for a retry to come due.
    pub(crate) async fn bank_loop(self, wake: std::sync::Arc<tokio::sync::Notify>) {
        loop {
            match self.next_outcome() {
                Ok(Some(o)) => self.write_outcome(o).await,
                Ok(None) => {
                    let _ = tokio::time::timeout(Duration::from_secs(30), wake.notified()).await;
                }
                Err(e) => {
                    eprintln!("kizn: bank writing: {e}");
                    let _ = tokio::time::timeout(Duration::from_secs(30), wake.notified()).await;
                }
            }
        }
    }

    /// The first outcome, in topic and outcome order, that's waiting, was
    /// left mid-write, or failed and is due a retry.
    fn next_outcome(&self) -> Result<Option<OutcomeRow>, AppError> {
        let now = self.now();
        for t in self.store.all_topics()?.into_iter().filter(|t| t.status == "confirmed") {
            for o in self.store.outcomes(&t.id)? {
                let due = match o.bank_state.as_str() {
                    "waiting" | "writing" => true,
                    "failed" => o.retry_at.as_deref().is_some_and(|r| r <= now.as_str()),
                    _ => false,
                };
                if due {
                    return Ok(Some(o));
                }
            }
        }
        Ok(None)
    }

    fn fail(&self, o: &OutcomeRow, why: &str, retry_in: chrono::Duration) {
        let retry = sql_time(self.deps.clock.now() + retry_in);
        let _ = self.store.set_bank_state(&o.id, "failed", Some(why), Some(&retry));
        self.changed(&o.topic_id);
    }

    async fn write_outcome(&self, o: OutcomeRow) {
        let _ = self.store.set_bank_state(&o.id, "writing", None, None);
        self.changed(&o.topic_id);
        let models = self.role_model(Role::Author).and_then(|a| Ok((a, self.role_model(Role::Checker)?)));
        let (author, checker) = match models {
            Ok(m) => m,
            Err(e) => return self.fail(&o, &e.to_string(), chrono::Duration::minutes(RETRY_AFTER_MINUTES)),
        };
        match self.fill(&o, &author, &checker).await {
            Ok(Filled::Full) => {
                let _ = self.store.set_bank_state(&o.id, "ready", None, None);
                self.changed(&o.topic_id);
            }
            Ok(Filled::NothingUsable) => self.fail(
                &o,
                "the author model hasn't written usable items for this outcome yet; kizn will try again later",
                chrono::Duration::hours(BACKOFF_HOURS),
            ),
            Err(e) => self.fail(&o, &e.to_string(), chrono::Duration::minutes(RETRY_AFTER_MINUTES)),
        }
    }

    async fn fill(&self, o: &OutcomeRow, author: &RoleModel, checker: &RoleModel) -> Result<Filled, ModelError> {
        let db = |e: kizn_store::StoreError| ModelError::Config(e.to_string());
        let topic =
            self.store.topic(&o.topic_id).map_err(db)?.ok_or_else(|| ModelError::Config("topic gone".into()))?;
        let target = Tier::parse(&o.target_tier).unwrap_or(Tier::Understand);
        let concepts = self.store.concepts(&o.id).map_err(db)?;
        let concept_ids: Vec<String> = concepts.iter().map(|c| c.id.clone()).collect();
        let all_topics = self.store.all_topics().map_err(db)?;
        let mut empty_batches = 0;
        // A slot is filled to its full size before moving on. (bank_needs
        // only reports a slot while it's below the refill line, so asking it
        // again after every batch would stop each slot halfway.)
        let mut filling: Option<(kizn_core::bank::Slot, usize)> = None;
        loop {
            let active = self.store.items(&o.id, Some("active")).map_err(db)?;
            let banked: Vec<BankedItem> = active
                .iter()
                .map(|i| BankedItem {
                    id: &i.id,
                    kind: ItemKind::parse(&i.kind).unwrap_or(ItemKind::Recall),
                    concept_ids: &i.concept_ids,
                })
                .collect();
            // Practice (Phase 4) will mark items attempted; until then none are.
            let needs = bank_needs(target, &concept_ids, &banked, &HashSet::new());
            let next = filling.take().or_else(|| needs.into_iter().next());
            let Some((slot, missing)) = next else { return Ok(Filled::Full) };

            let count = missing.min(ITEMS_PER_CALL);
            let mut existing: Vec<(String, Vec<String>)> = self
                .store
                .items(&o.id, None)
                .map_err(db)?
                .into_iter()
                .filter(|i| i.status != "refused")
                .map(|i| (i.stem, i.concept_ids))
                .collect();
            let slot_concept = slot.concept_id.as_ref().and_then(|id| concepts.iter().find(|c| &c.id == id));
            let tier_ask = match slot.kind {
                ItemKind::Recall => "remember or understand".to_string(),
                ItemKind::Apply => Tier::apply_tier_for(target).name().to_string(),
                ItemKind::Target | ItemKind::Explain => target.name().to_string(),
            };
            let ask = format!(
                "<input>\n{}\n</input>",
                json!({
                    "topic": topic.title, "goal": topic.goal, "outcome": o.statement, "target_tier": target.name(),
                    "kind": slot.kind.name(), "tier": tier_ask, "count": count,
                    "concepts": concepts.iter().map(|c| c.statement.as_str()).collect::<Vec<_>>(),
                    "slot_concept": slot_concept.map(|c| c.statement.as_str()),
                    "existing_stems": existing.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(),
                    "topics": all_topics.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
                })
            );

            let batch = self.id();
            self.store.start_batch(&batch, &o.id, &author.provider, &self.now()).map_err(db)?;
            let mut counts = BatchCounts { asked: count as i64, ..Default::default() };
            let written = author.client.complete_json(WRITE_ITEMS, &ask, &items_schema(), author.mode).await;
            let (reply, usage) = match written {
                Ok(r) => r,
                Err(e) => {
                    let _ = self.store.finish_batch(&batch, &counts, Some(&e.to_string()), &self.now());
                    return Err(e);
                }
            };
            counts.prompt_tokens += usage.prompt_tokens as i64;
            counts.completion_tokens += usage.completion_tokens as i64;

            let concept_refs: Vec<Concept> =
                concepts.iter().map(|c| Concept { id: &c.id, name: &c.statement }).collect();
            let topic_refs: Vec<TopicRef> =
                all_topics.iter().map(|t| TopicRef { id: &t.id, title: &t.title }).collect();
            for draft in reply["items"].as_array().cloned().unwrap_or_default().iter().take(count) {
                let ctx = CheckContext {
                    kind: slot.kind,
                    target,
                    concepts: &concept_refs,
                    slot_concept_id: slot.concept_id.as_deref(),
                    existing: &existing,
                    topics: &topic_refs,
                    topic_id: &topic.id,
                };
                let (status, reason, item) = match check_item(draft, &ctx) {
                    Err(refusal) => ("refused", Some(refusal.to_string()), None),
                    Ok(item) => match &item.options {
                        None => ("active", None, Some(item)),
                        Some(options) => match self.blind(checker, &item.stem, options).await {
                            Ok(None) => ("active", None, Some(item)),
                            Ok(Some(why)) => ("refused", Some(format!("blind check: {why}")), Some(item)),
                            Err(e) => {
                                let _ = self.store.finish_batch(&batch, &counts, Some(&e.to_string()), &self.now());
                                return Err(e);
                            }
                        },
                    },
                };
                let row = ItemRow {
                    id: self.id(),
                    topic_id: topic.id.clone(),
                    outcome_id: o.id.clone(),
                    batch_id: Some(batch.clone()),
                    kind: slot.kind.name().into(),
                    tier: item.as_ref().map(|i| i.tier.name()).unwrap_or(target.name()).into(),
                    concept_ids: item
                        .as_ref()
                        .map(|i| i.concept_ids.clone())
                        .unwrap_or_else(|| slot.concept_id.clone().into_iter().collect()),
                    stem: item
                        .as_ref()
                        .map(|i| i.stem.clone())
                        .unwrap_or_else(|| draft["stem"].as_str().unwrap_or("").to_string()),
                    options_json: item
                        .as_ref()
                        .and_then(|i| i.options.as_ref())
                        .map(|o| serde_json::to_string(o).expect("options serialise")),
                    exemplar: item.as_ref().and_then(|i| i.exemplar.clone()),
                    source_ref: item.as_ref().and_then(|i| i.source_ref.clone()),
                    model_knowledge: item.as_ref().is_none_or(|i| i.model_knowledge),
                    status: status.into(),
                    status_reason: reason,
                    created_at: self.now(),
                };
                self.store.insert_item(&row).map_err(db)?;
                if status == "active" {
                    counts.accepted += 1;
                    existing.push((row.stem.clone(), row.concept_ids.clone()));
                    self.changed(&o.topic_id);
                } else {
                    counts.refused += 1;
                }
            }
            self.store.finish_batch(&batch, &counts, None, &self.now()).map_err(db)?;
            let left = missing.saturating_sub(counts.accepted as usize);
            if left > 0 {
                filling = Some((slot, left));
            }
            empty_batches = if counts.accepted == 0 { empty_batches + 1 } else { 0 };
            if empty_batches >= 2 {
                return Ok(Filled::NothingUsable);
            }
        }
    }

    /// The checker answers the item blind (stem and shuffled options, no
    /// key, no whys) in a fresh call. `None` when it passes.
    async fn blind(
        &self,
        checker: &RoleModel,
        stem: &str,
        options: &[kizn_core::bank::ItemOption],
    ) -> Result<Option<String>, ModelError> {
        let order = shuffled_order(options.len(), self.deps.rng.as_ref());
        let shown: Vec<String> = order
            .iter()
            .enumerate()
            .map(|(i, &j)| format!("{}. {}", LETTERS.chars().nth(i).unwrap_or('?'), options[j].text))
            .collect();
        let ask = format!("<input>\n{}\n</input>", json!({ "stem": stem, "options": shown }));
        let (reply, _) = checker.client.complete_json(BLIND_CHECK, &ask, &blind_schema(), checker.mode).await?;
        Ok(match parse_blind(&reply, &order) {
            Some((pick, also)) => blind_verdict(options, pick, &also),
            None => Some(format!("the checker's answer wasn't a letter: {reply}")),
        })
    }
}

enum Filled {
    Full,
    NothingUsable,
}
