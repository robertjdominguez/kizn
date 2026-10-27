//! Writing the bank: closed items per outcome, written by the author model in
//! the background, checked in code, then answered blind by the checker model
//! in a fresh call. An item goes live only if both pass; a refused one is kept
//! with its reason and never shown. Nobody waits on this, so it can be slow
//! and retried.

use std::collections::HashSet;
use std::time::Duration;

use kizn_core::bank::{
    BankedItem, CheckContext, Concept, ItemKind, LETTERS, OutcomeBank, Slot, TopicRef, bank_needs, blind_verdict,
    check_item, next_need, parse_blind, shuffled_order,
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
const BACKOFF_HOURS: i64 = 1;
/// Runs in a row that accept nothing before an outcome backs off. Local
/// models are inconsistent rather than unable; a few tries is fair.
const EMPTY_RUNS_BEFORE_BACKOFF: usize = 4;

const WRITE_ITEMS: &str = "kizn-task: write-items
You write practice items for one adult learner. They are shown later, one at a time, as cards; nobody is waiting on you. Code checks every item, then another model answers each one blind, so write items with one clearly best answer.

Rules for every scored item (recall, apply, target):
- Ask only about the concepts given, as their statements put them. Nothing outside them.
- Three to five options. Exactly one correct. The others are plausible to someone holding a real misconception, not wordplay or trick wording.
- Every option has \"why\": one line on why it's right, or what's wrong with it.
- Every wrong option has \"misconception\": the mistaken belief it stands for, in one line.
- Every wrong option has \"foundation\": if its misconception is really a gap in a more basic topic (one of the topics listed, or another more basic one), name that topic; otherwise \"none\".
- Keep each stem under 1,200 characters and each option under 250 (a line, not a paragraph): longer ones are refused.
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

/// One batch to write: an outcome's slot, and how many it still needs.
struct Job {
    outcome: OutcomeRow,
    slot: Slot,
    count: usize,
}

impl Ctx {
    /// Runs forever: writes the next batch the learner will need, or waits to
    /// be woken (a confirm, a retry) or for a retry to come due.
    pub(crate) async fn bank_loop(self, wake: std::sync::Arc<tokio::sync::Notify>) {
        loop {
            match self.next_job() {
                Ok(Some(job)) => self.run_job(job).await,
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

    /// Settles every confirmed topic's outcome states (ready when nothing's
    /// needed, waiting when something is; failed ones wait for their retry),
    /// then returns the first batch to write: the oldest topic that needs
    /// one, in the order its learner will need it (kizn_core::bank::next_need).
    /// Worked out from what's stored, so a restart carries straight on.
    fn next_job(&self) -> Result<Option<Job>, AppError> {
        let now = self.now();
        let attempted: HashSet<String> =
            self.store.attempts(None)?.into_iter().filter(|a| a.disputed_at.is_none()).map(|a| a.item_id).collect();
        for t in self.store.all_topics()?.into_iter().filter(|t| t.status == "confirmed") {
            let outcomes = self.store.outcomes(&t.id)?;
            let mut data = vec![];
            for o in &outcomes {
                let concept_ids: Vec<String> = self.store.concepts(&o.id)?.into_iter().map(|c| c.id).collect();
                let items = self.store.items(&o.id, Some("active"))?;
                data.push((o, concept_ids, items));
            }
            let mut moved = false;
            let mut eligible = vec![];
            for (o, concept_ids, items) in &data {
                let paused = o.bank_state == "failed" && o.retry_at.as_deref().is_some_and(|r| r > now.as_str());
                if paused {
                    continue;
                }
                let banked = banked(items);
                let target = Tier::parse(&o.target_tier).unwrap_or(Tier::Understand);
                let state =
                    if bank_needs(target, concept_ids, &banked, &attempted).is_empty() { "ready" } else { "waiting" };
                if o.bank_state != state {
                    self.store.set_bank_state(&o.id, state, None, None)?;
                    moved = true;
                }
                eligible.push((*o, concept_ids, items));
            }
            if moved {
                self.changed(&t.id);
            }
            let banks: Vec<OutcomeBank> = eligible
                .iter()
                .map(|(o, concept_ids, items)| OutcomeBank {
                    target: Tier::parse(&o.target_tier).unwrap_or(Tier::Understand),
                    concept_ids,
                    active: banked(items),
                    attempted: &attempted,
                })
                .collect();
            if let Some((i, slot, count)) = next_need(&banks) {
                return Ok(Some(Job { outcome: eligible[i].0.clone(), slot, count }));
            }
        }
        Ok(None)
    }

    /// Waits (up to a few minutes) while the tutor is answering on any of
    /// these providers.
    async fn wait_for_tutor(&self, providers: &[&str]) {
        for _ in 0..900 {
            let busy = self.tutoring.lock().unwrap().values().any(|p| providers.contains(&p.as_str()));
            if !busy {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Up to three resources kizn holds for a concept (or, for a scenario
    /// slot, the outcome's concepts), each with the excerpts that best match.
    fn resources_for(&self, concept: Option<&str>, concepts: &[kizn_store::ConceptRow]) -> Vec<Value> {
        let ids: Vec<&kizn_store::ConceptRow> = match concept {
            Some(id) => concepts.iter().filter(|c| c.id == id).collect(),
            None => concepts.iter().collect(),
        };
        let mut out = vec![];
        for c in ids {
            for s in self.store.sources(&c.id).unwrap_or_default() {
                if out.len() >= 3 {
                    return out;
                }
                if s.status != "verified" || s.content_json.is_none() {
                    continue;
                }
                let content = s.content_json.as_deref().unwrap_or("[]");
                let chunks = if s.kind == "video" {
                    let cues: Vec<kizn_core::source::Cue> = serde_json::from_str(content).unwrap_or_default();
                    kizn_core::source::cue_chunks(&cues, 60.0)
                } else {
                    let blocks: Vec<kizn_core::source::Block> = serde_json::from_str(content).unwrap_or_default();
                    kizn_core::source::page_chunks(&blocks, 120)
                };
                let picked: Vec<String> =
                    kizn_core::source::relevant(&chunks, &c.statement, 2).into_iter().map(|ch| ch.text).collect();
                out.push(json!({ "title": s.title, "url": s.url, "concept": c.statement, "excerpts": picked }));
            }
        }
        out
    }

    fn fail(&self, o: &OutcomeRow, why: &str, retry_in: chrono::Duration) {
        let retry = sql_time(self.deps.clock.now() + retry_in);
        let _ = self.store.set_bank_state(&o.id, "failed", Some(why), Some(&retry));
        self.changed(&o.topic_id);
    }

    async fn run_job(&self, job: Job) {
        let o = &job.outcome;
        let _ = self.store.set_bank_state(&o.id, "writing", None, None);
        self.changed(&o.topic_id);
        let models = self.role_model(Role::Author).and_then(|a| Ok((a, self.role_model(Role::Checker)?)));
        let (author, checker) = match models {
            Ok(m) => m,
            Err(e) => return self.fail(o, &e.to_string(), chrono::Duration::minutes(RETRY_AFTER_MINUTES)),
        };
        // The learner comes first: while the tutor answers on the same model,
        // the writer waits before its next call.
        self.wait_for_tutor(&[author.provider.as_str(), checker.provider.as_str()]).await;
        match self.write_batch(o, &job.slot, job.count, &author, &checker).await {
            Err(e) => self.fail(o, &e.to_string(), chrono::Duration::minutes(RETRY_AFTER_MINUTES)),
            Ok(_) => {
                // Several runs in a row with nothing usable: leave it a while.
                let recent =
                    self.store.recent_batch_accepts(&o.id, EMPTY_RUNS_BEFORE_BACKOFF as i64).unwrap_or_default();
                if recent.len() >= EMPTY_RUNS_BEFORE_BACKOFF && recent.iter().all(|n| *n == 0) {
                    return self.fail(
                        o,
                        "the author model hasn't written usable items for this outcome yet; kizn will try again later",
                        chrono::Duration::hours(BACKOFF_HOURS),
                    );
                }
                // Settled (ready or waiting) by the next pass.
                let _ = self.store.set_bank_state(&o.id, "waiting", None, None);
                self.changed(&o.topic_id);
            }
        }
    }

    /// Writes one batch for a slot: the author writes, the code checks, the
    /// checker answers each scored item blind. Returns how many went live.
    async fn write_batch(
        &self,
        o: &OutcomeRow,
        slot: &Slot,
        count: usize,
        author: &RoleModel,
        checker: &RoleModel,
    ) -> Result<i64, ModelError> {
        let db = |e: kizn_store::StoreError| ModelError::Config(e.to_string());
        let topic =
            self.store.topic(&o.topic_id).map_err(db)?.ok_or_else(|| ModelError::Config("topic gone".into()))?;
        let target = Tier::parse(&o.target_tier).unwrap_or(Tier::Understand);
        let concepts = self.store.concepts(&o.id).map_err(db)?;
        let all_topics = self.store.all_topics().map_err(db)?;
        let count = count.min(ITEMS_PER_CALL);
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
        // The rules that refuse most items, said concretely for this
        // request: models follow a named constraint better than a general one.
        let concept_rule = match (slot.kind, slot_concept) {
            (ItemKind::Recall, Some(c)) => format!(
                "Each item tests exactly one concept: \"{}\". Set \"concepts\" to [\"{}\"] and nothing else. \
                 (kizn counts each answer as evidence about that one concept, so an item that brings in another \
                 concept is refused.) The tier is remember or understand.",
                c.statement, c.statement
            ),
            (ItemKind::Explain, _) => "Write explain items: a prompt and an exemplar, no options.".to_string(),
            (kind, _) => format!(
                "Each item is a scenario that needs at least two of the concepts listed, named in \"concepts\" \
                 exactly as written there. The tier is {tier_ask}.{}",
                if kind == ItemKind::Target { " It's the outcome's target level." } else { "" }
            ),
        };
        // The resources kizn holds for the slot's concept(s): write from them,
        // and cite them.
        let resources = self.resources_for(slot_concept.map(|c| c.id.as_str()), &concepts);
        let cite_rule = if resources.is_empty() {
            String::new()
        } else {
            " Write from the excerpts of the resources listed below: set \"source\" to the title of the resource you \
             used, exactly as given, and \"model_knowledge\" to false."
                .to_string()
        };
        let ask = format!(
            "Write {count} {} item{}. {concept_rule}{cite_rule} A wrong option's \"foundation\" names a more basic \
             topic than \"{}\", never \"{}\" itself (or \"none\").\n\n<input>\n{}\n</input>",
            slot.kind.name(),
            if count == 1 { "" } else { "s" },
            topic.title,
            topic.title,
            json!({
                "topic": topic.title, "goal": topic.goal, "outcome": o.statement, "target_tier": target.name(),
                "kind": slot.kind.name(), "tier": tier_ask, "count": count,
                "concepts": concepts.iter().map(|c| c.statement.as_str()).collect::<Vec<_>>(),
                "slot_concept": slot_concept.map(|c| c.statement.as_str()),
                "existing_stems": existing.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(),
                "topics": all_topics.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
                "resources": resources,
            })
        );

        let batch = self.id();
        // The learner comes first: the batch starts only while the tutor isn't
        // answering on these models, checked and started under the lock that
        // registers a tutor reply, so a question can't slip in between.
        let providers = [author.provider.as_str(), checker.provider.as_str()];
        loop {
            {
                let tutoring = self.tutoring.lock().unwrap();
                if !tutoring.values().any(|p| providers.contains(&p.as_str())) {
                    self.store.start_batch(&batch, &o.id, &author.provider, &self.now()).map_err(db)?;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
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

        let concept_refs: Vec<Concept> = concepts.iter().map(|c| Concept { id: &c.id, name: &c.statement }).collect();
        let topic_refs: Vec<TopicRef> = all_topics.iter().map(|t| TopicRef { id: &t.id, title: &t.title }).collect();
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
            let given: Vec<&str> = resources.iter().filter_map(|r| r["title"].as_str()).collect();
            let cited: Option<String> = item
                .as_ref()
                .and_then(|i| i.source_ref.as_deref())
                .and_then(|claim| given.iter().find(|t| t.eq_ignore_ascii_case(claim.trim())).map(|t| t.to_string()));
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
                // A citation counts only if it names a resource kizn gave it.
                source_ref: cited.clone(),
                model_knowledge: cited.is_none(),
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
        Ok(counts.accepted)
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

fn banked(items: &[ItemRow]) -> Vec<BankedItem<'_>> {
    items
        .iter()
        .map(|i| BankedItem {
            id: &i.id,
            kind: ItemKind::parse(&i.kind).unwrap_or(ItemKind::Recall),
            concept_ids: &i.concept_ids,
        })
        .collect()
}
