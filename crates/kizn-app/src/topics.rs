//! Starting a topic: a goal becomes a draft (outcomes, then concepts, drafted
//! by the author model in the background), the learner shapes it, and
//! confirming queues the bank. The model proposes; nothing it drafts is
//! final until the learner confirms.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kizn_core::bank::{ItemKind, MAX_CONCEPTS, MAX_OUTCOMES, bank_size, slots_for};
use kizn_core::{Tier, new_id, sql_time};
use kizn_model::{Client, JsonMode};
use kizn_store::{ConceptRow, OutcomeRow, TopicRow};
use serde_json::{Value, json};

use crate::command::{ConceptView, DraftInput, OutcomeView, Role, SlotView, TopicSummary, TopicView};
use crate::config::Config;
use crate::{App, AppError, Ctx, Event};

// ---- prompts ---------------------------------------------------------------------

const DRAFT_OUTCOMES: &str = "kizn-task: draft-outcomes
You help one adult learner turn what they need to be able to do into a short set of learning outcomes. They are learning for work, often next to a field they already know.

Write 3 to 5 outcomes:
- \"statement\": what they will be able to DO, starting with a verb (Explain, Diagnose, Compare, Advise, Design…). Concrete to their goal, not a syllabus heading.
- \"tier\": the revised-Bloom tier the goal asks for: remember, understand, apply, analyze, evaluate or create. Set it from what the goal demands (\"explain to customers\" is understand or analyze; \"advise\", \"defend\", \"be the expert\" is evaluate; \"design\" is create), never from what they know now.
- \"reason\": one line that quotes the words of their goal that set the tier.

Reply with JSON only: {\"outcomes\": [{\"statement\": \"...\", \"tier\": \"...\", \"reason\": \"...\"}]}";

const DRAFT_CONCEPTS: &str = "kizn-task: draft-concepts
List the concepts a learner needs to know to reach one outcome: 3 to 7 of them.
Each is one short statement of an idea or fact, as you'd want it understood (\"An image is immutable layers plus config\"), not a topic heading (\"Images\"). Only what this outcome needs; the other outcomes are listed so you don't repeat them.

Reply with JSON only: {\"concepts\": [\"...\", \"...\"]}";

fn input(v: &Value) -> String {
    format!("<input>\n{v}\n</input>")
}

fn outcomes_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"outcomes": {"type": "array", "items": {
            "type": "object",
            "properties": {"statement": {"type": "string"}, "tier": {"type": "string"}, "reason": {"type": "string"}},
            "required": ["statement", "tier", "reason"], "additionalProperties": false
        }}},
        "required": ["outcomes"], "additionalProperties": false
    })
}

fn concepts_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"concepts": {"type": "array", "items": {"type": "string"}}},
        "required": ["concepts"], "additionalProperties": false
    })
}

// ---- the model for a role ----------------------------------------------------------

/// How long one drafting or writing request may take. Local models (and
/// thinking models) can take minutes to write a batch; the connection test
/// keeps its short limit.
const JOB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

pub(crate) struct RoleModel {
    pub provider: String,
    pub client: Client,
    pub mode: JsonMode,
}

impl Ctx {
    /// The model a role points at, ready to call: native JSON if its last
    /// test showed schema support, else prompted.
    pub(crate) fn role_model(&self, role: Role) -> Result<RoleModel, AppError> {
        let config = Config::load(&self.paths.config_dir)?;
        let name = config.roles.get(role).cloned().ok_or_else(|| {
            AppError::Refused(format!("no model is set up as the {}: connect one in Settings", role.key()))
        })?;
        let p = config.providers.get(&name).ok_or_else(|| {
            AppError::Refused(format!("the {} is set to {name}, which isn't in config.toml", role.key()))
        })?;
        let endpoint = crate::settings::endpoint(self.deps.secrets.as_ref(), &name, p)?;
        let prompted = self.store.checks()?.iter().any(|c| c.provider == name && c.check_json.contains("\"prompted\""));
        Ok(RoleModel {
            provider: name,
            client: Client::new(endpoint, self.deps.http.clone()).with_timeout(JOB_TIMEOUT),
            mode: if prompted { JsonMode::Prompted } else { JsonMode::Native },
        })
    }

    pub(crate) fn now(&self) -> String {
        sql_time(self.deps.clock.now())
    }

    pub(crate) fn id(&self) -> String {
        new_id(self.deps.clock.as_ref(), self.deps.rng.as_ref())
    }

    pub(crate) fn changed(&self, topic_id: &str) {
        let _ = self.events.send(Event::TopicChanged { topic_id: topic_id.to_string() });
    }
}

// ---- drafting ----------------------------------------------------------------------

/// Drafts a topic in the background: outcomes first (each one an event, so
/// they appear one by one), then concepts outcome by outcome. Stops between
/// calls when asked.
async fn draft(ctx: Ctx, topic: TopicRow, stop: Arc<AtomicBool>) {
    if let Err(e) = draft_inner(&ctx, &topic, &stop).await {
        let _ = ctx.store.set_draft_state(&topic.id, "failed", Some(&e.to_string()));
    }
    ctx.changed(&topic.id);
}

async fn draft_inner(ctx: &Ctx, topic: &TopicRow, stop: &AtomicBool) -> Result<(), AppError> {
    let author = ctx.role_model(Role::Author)?;
    // Resuming: keep the outcomes already drafted and carry on with their
    // concepts; only a draft without outcomes asks for them.
    let existing = ctx.store.outcomes(&topic.id)?;
    let outcomes = if existing.is_empty() { draft_outcomes(ctx, topic, &author).await? } else { existing };
    for o in outcomes.iter().filter(|o| o.concepts_state != "done") {
        if stop.load(Ordering::Relaxed) {
            ctx.store.set_draft_state(&topic.id, "stopped", None)?;
            return Ok(());
        }
        ctx.store.set_concepts_state(&o.id, "drafting")?;
        ctx.changed(&topic.id);
        let others: Vec<&str> = outcomes.iter().filter(|x| x.id != o.id).map(|x| x.statement.as_str()).collect();
        let ask = input(&json!({
            "title": topic.title, "goal": topic.goal, "outcome": o.statement, "tier": o.target_tier, "other_outcomes": others
        }));
        let reply = author.client.complete_json(DRAFT_CONCEPTS, &ask, &concepts_schema(), author.mode).await;
        let (reply, _) = match reply {
            Ok(r) => r,
            Err(e) => {
                ctx.store.set_concepts_state(&o.id, "pending")?;
                return Err(AppError::Refused(format!("the author model couldn't draft concepts: {e}")));
            }
        };
        let concepts = reply["concepts"].as_array().cloned().unwrap_or_default();
        for (i, c) in concepts
            .iter()
            .filter_map(|c| c.as_str())
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .take(MAX_CONCEPTS)
            .enumerate()
        {
            ctx.store.insert_concept(&ConceptRow {
                id: ctx.id(),
                outcome_id: o.id.clone(),
                ord: i as i64 + 1,
                statement: c.to_string(),
            })?;
        }
        ctx.store.set_concepts_state(&o.id, "done")?;
        ctx.changed(&topic.id);
    }
    // Foundations last: proposed once the outcomes are known, never failing
    // the draft.
    ctx.draft_foundations(topic, &author).await?;
    ctx.store.set_draft_state(&topic.id, "drafted", None)?;
    Ok(())
}

async fn draft_outcomes(ctx: &Ctx, topic: &TopicRow, author: &RoleModel) -> Result<Vec<OutcomeRow>, AppError> {
    let ask = input(&json!({"title": topic.title, "goal": topic.goal}));
    let (reply, _) = author
        .client
        .complete_json(DRAFT_OUTCOMES, &ask, &outcomes_schema(), author.mode)
        .await
        .map_err(|e| AppError::Refused(format!("the author model couldn't draft outcomes: {e}")))?;
    let drafted: Vec<&Value> = reply["outcomes"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let mut outcomes = vec![];
    for (i, o) in drafted.into_iter().take(MAX_OUTCOMES).enumerate() {
        let statement = o["statement"].as_str().unwrap_or("").trim().to_string();
        if statement.is_empty() {
            continue;
        }
        let tier = o["tier"].as_str().and_then(Tier::parse).unwrap_or(Tier::Understand);
        let row = OutcomeRow {
            id: ctx.id(),
            topic_id: topic.id.clone(),
            ord: i as i64 + 1,
            statement,
            target_tier: tier.name().into(),
            tier_reason: o["reason"].as_str().unwrap_or("").trim().to_string(),
            concepts_state: "pending".into(),
            bank_state: "waiting".into(),
            bank_error: None,
            retry_at: None,
        };
        ctx.store.insert_outcome(&row)?;
        ctx.changed(&topic.id);
        outcomes.push(row);
    }
    if outcomes.is_empty() {
        return Err(AppError::Refused("the author model's reply had no outcomes in it".into()));
    }
    Ok(outcomes)
}

// ---- commands ------------------------------------------------------------------------

impl App {
    /// Resolves a foundation: "start" sets up a new topic underneath (linked
    /// as a foundation, drafting under way) and returns it; "later" and
    /// "dismiss" return the topic it was for.
    pub(crate) fn resolve_foundation(&self, suggestion_id: &str, action: &str) -> Result<crate::Reply, AppError> {
        let s = self
            .store
            .suggestions(None)?
            .into_iter()
            .find(|s| s.id == suggestion_id)
            .ok_or_else(|| AppError::Refused("there's no such suggestion".into()))?;
        match action {
            "start" => {
                let for_topic =
                    self.store.topic(&s.for_topic)?.ok_or_else(|| AppError::Refused("its topic is gone".into()))?;
                let goal = match &s.why {
                    Some(why) => format!("the foundations for {}: {why}", for_topic.title),
                    None => format!("the foundations for {}", for_topic.title),
                };
                let started = self.start_topic(&s.title, &goal)?;
                self.link_topics(&s.for_topic, &started.id, "rests_on")?;
                self.store.set_suggestion(&s.id, "started", Some(&started.id))?;
                self.ctx.changed(&s.for_topic);
                self.topic_view(&started.id).map(crate::Reply::Topic)
            }
            "later" | "dismiss" => {
                let topic_id = self.set_aside_foundation(&s.id, action == "dismiss")?;
                self.topic_view(&topic_id).map(crate::Reply::Topic)
            }
            other => Err(AppError::Refused(format!("there's no way to resolve a foundation called {other:?}"))),
        }
    }

    pub(crate) fn start_topic(&self, title: &str, goal: &str) -> Result<TopicView, AppError> {
        let (title, goal) = (title.trim(), goal.trim());
        if title.is_empty() {
            return Err(AppError::Refused("a topic needs a name".into()));
        }
        if goal.is_empty() {
            return Err(AppError::Refused(
                "say what you want to be able to do, and why: kizn drafts the outcomes from it".into(),
            ));
        }
        // Fail now, in words, rather than in the background.
        self.ctx.role_model(Role::Author)?;
        let id = self.ctx.id();
        self.store.create_draft_topic(&id, title, goal, &self.ctx.now())?;
        let topic = self.store.topic(&id)?.expect("just created");
        let stop = Arc::new(AtomicBool::new(false));
        self.drafting.lock().unwrap().insert(id.clone(), stop.clone());
        tokio::spawn(draft(self.ctx.clone(), topic, stop));
        self.topic_view(&id)
    }

    /// Carries on a draft that failed or was stopped, from where it left off.
    pub(crate) fn resume_drafting(&self, topic_id: &str) -> Result<TopicView, AppError> {
        let topic = self.draft_topic(topic_id)?;
        if topic.draft_state == "drafting" {
            return self.topic_view(&topic.id);
        }
        self.ctx.role_model(Role::Author)?;
        self.store.set_draft_state(&topic.id, "drafting", None)?;
        let topic = self.store.topic(&topic.id)?.expect("still there");
        let stop = Arc::new(AtomicBool::new(false));
        self.drafting.lock().unwrap().insert(topic.id.clone(), stop.clone());
        tokio::spawn(draft(self.ctx.clone(), topic.clone(), stop));
        self.ctx.changed(&topic.id);
        self.topic_view(&topic.id)
    }

    pub(crate) fn stop_drafting(&self, topic_id: &str) -> Result<TopicView, AppError> {
        if let Some(stop) = self.drafting.lock().unwrap().get(topic_id) {
            stop.store(true, Ordering::Relaxed);
        }
        self.topic_view(topic_id)
    }

    /// Replaces a draft with the learner's shaped version.
    pub(crate) fn save_draft(&self, topic_id: &str, draft: &DraftInput) -> Result<TopicView, AppError> {
        let topic = self.draft_topic(topic_id)?;
        let mut rows = vec![];
        for (i, o) in draft.outcomes.iter().enumerate() {
            let statement = o.statement.trim();
            if statement.is_empty() {
                return Err(AppError::Refused(format!("outcome {} has no wording", i + 1)));
            }
            let tier = Tier::parse(&o.target)
                .or_else(|| Tier::from_plain(&o.target))
                .ok_or_else(|| AppError::Refused(format!("\"{}\" isn't a target kizn knows", o.target)))?;
            let concepts: Vec<&str> = o.concepts.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
            if concepts.len() > MAX_CONCEPTS {
                return Err(AppError::Refused(format!("outcome {} has more than {MAX_CONCEPTS} concepts", i + 1)));
            }
            let outcome_id = o.id.clone().filter(|id| !id.is_empty()).unwrap_or_else(|| self.ctx.id());
            let outcome = OutcomeRow {
                id: outcome_id.clone(),
                topic_id: topic.id.clone(),
                ord: i as i64 + 1,
                statement: statement.to_string(),
                target_tier: tier.name().into(),
                tier_reason: o.reason.clone(),
                concepts_state: "done".into(),
                bank_state: "waiting".into(),
                bank_error: None,
                retry_at: None,
            };
            let concepts = concepts
                .iter()
                .enumerate()
                .map(|(j, c)| ConceptRow {
                    id: self.ctx.id(),
                    outcome_id: outcome_id.clone(),
                    ord: j as i64 + 1,
                    statement: c.to_string(),
                })
                .collect();
            rows.push((outcome, concepts));
        }
        self.store.replace_draft(&topic.id, &rows)?;
        self.ctx.changed(&topic.id);
        self.topic_view(&topic.id)
    }

    pub(crate) fn confirm_topic(&self, topic_id: &str) -> Result<TopicView, AppError> {
        let topic = self.draft_topic(topic_id)?;
        if topic.draft_state == "drafting" {
            return Err(AppError::Refused("kizn is still drafting this topic: wait, or stop it first".into()));
        }
        let outcomes = self.store.outcomes(&topic.id)?;
        if outcomes.is_empty() {
            return Err(AppError::Refused("a topic needs at least one outcome".into()));
        }
        for o in &outcomes {
            if self.store.concepts(&o.id)?.is_empty() {
                return Err(AppError::Refused(format!("outcome {} has no concepts yet", o.ord)));
            }
            self.store.set_bank_state(&o.id, "waiting", None, None)?;
        }
        self.store.confirm_topic(&topic.id, &self.ctx.now())?;
        self.link_kept_foundations(&topic.id)?;
        self.start_ground(&topic.id);
        self.drafting.lock().unwrap().remove(&topic.id);
        self.ctx.changed(&topic.id);
        self.wake_bank();
        self.topic_view(&topic.id)
    }

    fn draft_topic(&self, topic_id: &str) -> Result<TopicRow, AppError> {
        let topic = self.store.topic(topic_id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?;
        if topic.status != "draft" {
            return Err(AppError::Refused("a confirmed topic can't be changed".into()));
        }
        Ok(topic)
    }

    pub(crate) fn topic_view(&self, topic_id: &str) -> Result<TopicView, AppError> {
        let topic = self.store.topic(topic_id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?;
        let active = self.store.active_counts(&topic.id)?;
        let standings = if topic.status == "confirmed" { self.standings(&topic.id)? } else { Default::default() };
        let mut outcomes = vec![];
        for o in self.store.outcomes(&topic.id)? {
            let concepts: Vec<ConceptView> = self
                .store
                .concepts(&o.id)?
                .into_iter()
                .map(|c| {
                    let state = standings.get(&c.id).map(|s| s.state.name()).unwrap_or("unseen").to_string();
                    let studied = self.store.studied_at(&c.id).ok().flatten().is_some();
                    ConceptView { id: c.id, statement: c.statement, state, studied }
                })
                .collect();
            let tier = Tier::parse(&o.target_tier).unwrap_or(Tier::Understand);
            let n = active.iter().find(|(id, _)| *id == o.id).map(|(_, n)| *n).unwrap_or(0);
            let items = self.store.items(&o.id, Some("active"))?;
            let concept_ids: Vec<String> = concepts.iter().map(|c| c.id.clone()).collect();
            let slots = slots_for(tier, &concept_ids)
                .into_iter()
                .map(|slot| {
                    let have = items
                        .iter()
                        .filter(|i| {
                            ItemKind::parse(&i.kind) == Some(slot.kind)
                                && slot.concept_id.as_ref().is_none_or(|c| i.concept_ids.first() == Some(c))
                        })
                        .count();
                    SlotView {
                        kind: slot.kind.name().into(),
                        concept: slot
                            .concept_id
                            .as_ref()
                            .and_then(|id| concepts.iter().find(|c| &c.id == id))
                            .map(|c| c.statement.clone()),
                        have: have.min(slot.kind.slot_size()) as i64,
                        size: slot.kind.slot_size() as i64,
                    }
                })
                .collect();
            outcomes.push(OutcomeView {
                expected: bank_size(tier, concepts.len()) as i64,
                id: o.id,
                ord: o.ord,
                statement: o.statement,
                target: tier.name().into(),
                target_plain: tier.plain().into(),
                reason: o.tier_reason,
                concepts_state: o.concepts_state,
                bank_state: o.bank_state,
                bank_error: o.bank_error,
                active: n,
                concepts,
                slots,
            });
        }
        let p = if topic.status == "confirmed" { self.practice_state(&topic.id)? } else { Default::default() };
        let links = self.topic_links(&topic.id)?;
        Ok(TopicView {
            foundations: self.suggestion_views(&topic.id)?,
            flags: if topic.status == "confirmed" { self.flags(&topic.id)? } else { vec![] },
            rests_on: links.rests_on,
            supports: links.supports,
            parts: links.parts,
            part_of: links.part_of,
            layer: links.layer,
            can_practise: p.due + p.new > 0,
            practised: p.practised,
            next_due_in_days: p.next_due_in_days,
            due_now: p.due,
            new_now: p.new,
            new_outcome_ord: p.new_outcome_ord,
            active: outcomes.iter().map(|o| o.active).sum(),
            expected: outcomes.iter().map(|o| o.expected).sum(),
            id: topic.id,
            title: topic.title,
            goal: topic.goal,
            status: topic.status,
            draft_state: topic.draft_state,
            draft_error: topic.draft_error,
            outcomes,
        })
    }

    pub(crate) fn topic_summaries(&self) -> Result<Vec<TopicSummary>, AppError> {
        let mut out = vec![];
        for t in self.store.all_topics()? {
            let v = self.topic_view(&t.id)?;
            out.push(TopicSummary {
                id: v.id,
                title: v.title,
                status: v.status,
                preparing: v.outcomes.iter().any(|o| o.bank_state != "ready"),
                outcomes: v.outcomes.len() as i64,
                concepts: v.outcomes.iter().map(|o| o.concepts.len() as i64).sum(),
                active: v.active,
                expected: v.expected,
                states: v.outcomes.iter().flat_map(|o| o.concepts.iter().map(|c| c.state.clone())).collect(),
                due_now: v.due_now,
                new_now: v.new_now,
                practised: v.practised,
                next_due_in_days: v.next_due_in_days,
            });
        }
        Ok(out)
    }
}
