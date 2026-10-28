//! Practice: builds a set, records answers, says where things stand. No model
//! is involved anywhere here: the set is composed, shuffled and graded in
//! code (kizn_core::deck, kizn_core::progress), from the stored bank and the
//! answers.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Utc};
use kizn_core::bank::{BankedItem, ItemKind, ItemOption, placement_ready, shuffled_order};
use kizn_core::deck::{Candidate, DeckConcept, DeckOutcome, Reason, Today, compose};
use kizn_core::progress::{Answer, ConceptState, Standing, standing};
use kizn_core::{parse_sql_time, sql_time};
use kizn_store::{AttemptRow, ConceptRow, ItemRow, OutcomeRow, TopicRow};

use crate::command::{
    AnswerView, CardView, ConceptChange, ConceptLine, ConceptProgress, DayView, DeckView, DisputeView, NextStep,
    OptionFeedback, OutcomeProgress, OutcomeStanding, ProgressView, StateCounts, SummaryView,
};
use crate::{AppError, Ctx};

/// Everything practice works from, for some topics: their outcomes in order,
/// concepts, active items and the answers so far.
struct Ground {
    topics: Vec<TopicRow>,
    outcomes: Vec<(OutcomeRow, Vec<ConceptRow>, Vec<ItemRow>)>,
    attempts: Vec<AttemptRow>,
    /// Item id → the concepts it names, for every item answered.
    item_concepts: HashMap<String, Vec<String>>,
    /// The learner's offset from UTC, for where a day starts.
    offset: chrono::FixedOffset,
}

impl Ground {
    /// Each concept's answers, oldest first (disputed ones left out).
    fn answers(&self) -> HashMap<String, Vec<Answer>> {
        let mut by: HashMap<String, Vec<Answer>> = HashMap::new();
        for a in self.attempts.iter().filter(|a| a.disputed_at.is_none()) {
            let Some(at) = parse_sql_time(&a.at) else { continue };
            for c in self.item_concepts.get(&a.item_id).into_iter().flatten() {
                by.entry(c.clone()).or_default().push(Answer { item_id: a.item_id.clone(), correct: a.correct, at });
            }
        }
        by
    }

    fn standings(&self) -> HashMap<String, Standing> {
        self.answers().into_iter().map(|(c, a)| (c, standing(&a))).collect()
    }

    fn last_shown(&self) -> HashMap<String, DateTime<Utc>> {
        let mut last: HashMap<String, DateTime<Utc>> = HashMap::new();
        for a in &self.attempts {
            if let Some(at) = parse_sql_time(&a.at) {
                let e = last.entry(a.item_id.clone()).or_insert(at);
                *e = (*e).max(at);
            }
        }
        last
    }

    /// What was practised today, for the daily caps: recall answers that
    /// weren't a concept's first, and concepts first answered today.
    fn today(&self, now: DateTime<Utc>) -> Today {
        let answers = self.answers();
        let day = |t: DateTime<Utc>| t.with_timezone(&self.offset).date_naive();
        let on_today = |a: &Answer| day(a.at) == day(now);
        Today {
            recalls: answers.values().flat_map(|a| a.iter().skip(1)).filter(|a| on_today(a)).count(),
            new: answers.values().filter(|a| a.first().is_some_and(on_today)).count(),
        }
    }

    /// The deck's view of these outcomes. Placement is only offered on an
    /// outcome once every concept on it has a question (one set, not a
    /// trickle of single cards).
    fn deck_outcomes(&self) -> Vec<DeckOutcome> {
        let standings = self.standings();
        let last = self.last_shown();
        self.outcomes
            .iter()
            .map(|(o, concepts, items)| {
                let ids: Vec<String> = concepts.iter().map(|c| c.id.clone()).collect();
                let ready = placement_ready(&ids, &banked(items));
                DeckOutcome {
                    id: o.id.clone(),
                    concepts: concepts
                        .iter()
                        .map(|c| DeckConcept {
                            id: c.id.clone(),
                            standing: standings.get(&c.id).cloned().unwrap_or_default(),
                            recall: items
                                .iter()
                                .filter(|i| i.kind == "recall" && i.concept_ids.first() == Some(&c.id))
                                .map(|i| Candidate { item_id: i.id.clone(), last_shown: last.get(&i.id).copied() })
                                .collect(),
                        })
                        .filter(|c| ready || c.standing.state != ConceptState::Unseen)
                        .collect(),
                }
            })
            .collect()
    }

    fn concept(&self, id: &str) -> Option<(&OutcomeRow, &ConceptRow)> {
        self.outcomes.iter().find_map(|(o, cs, _)| cs.iter().find(|c| c.id == id).map(|c| (o, c)))
    }

    fn standing_of(
        &self,
        o: &OutcomeRow,
        concepts: &[ConceptRow],
        standings: &HashMap<String, Standing>,
    ) -> OutcomeStanding {
        let states: Vec<&Standing> = concepts.iter().filter_map(|c| standings.get(&c.id)).collect();
        let count = |s: ConceptState| states.iter().filter(|x| x.state == s).count() as i64;
        OutcomeStanding {
            outcome_id: o.id.clone(),
            ord: o.ord,
            statement: o.statement.clone(),
            concepts: concepts.len() as i64,
            unseen: concepts.len() as i64 - states.len() as i64,
            recalled: count(ConceptState::Recalled),
            retained: count(ConceptState::Retained),
            next_due: states.iter().filter_map(|s| s.due).min().map(sql_time),
        }
    }
}

/// What a topic's next set would hold, for its page.
#[derive(Debug, Default)]
pub(crate) struct PracticeState {
    pub due: i64,
    pub new: i64,
    pub new_outcome_ord: Option<i64>,
    pub practised: bool,
    pub next_due_in_days: Option<i64>,
}

/// Days from now until `t`, rounded up (1 is tomorrow; 0 if it's past).
pub(crate) fn days_until(t: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    ((t - now).num_seconds().max(0) + 86_399) / 86_400
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

fn options(item: &ItemRow) -> Result<Vec<ItemOption>, AppError> {
    let json = item.options_json.as_deref().ok_or_else(|| AppError::Refused("that question has no options".into()))?;
    serde_json::from_str(json).map_err(|e| AppError::Io(format!("a stored question's options don't read: {e}")))
}

impl Ctx {
    /// One topic's ground, or every confirmed topic's (oldest first).
    fn ground(&self, topic_id: Option<&str>) -> Result<Ground, AppError> {
        let topics: Vec<TopicRow> = match topic_id {
            Some(id) => {
                vec![self.store.topic(id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?]
            }
            None => self.store.all_topics()?.into_iter().filter(|t| t.status == "confirmed").collect(),
        };
        let mut outcomes = vec![];
        let mut attempts = vec![];
        for t in &topics {
            for o in self.store.outcomes(&t.id)? {
                let concepts = self.store.concepts(&o.id)?;
                let items = self.store.items(&o.id, Some("active"))?;
                outcomes.push((o, concepts, items));
            }
            attempts.extend(self.store.attempts(Some(&t.id))?);
        }
        let mut item_concepts = HashMap::new();
        for a in &attempts {
            if !item_concepts.contains_key(&a.item_id)
                && let Some(item) = self.store.item(&a.item_id)?
            {
                item_concepts.insert(a.item_id.clone(), item.concept_ids);
            }
        }
        Ok(Ground { topics, outcomes, attempts, item_concepts, offset: self.deps.utc_offset })
    }

    pub(crate) fn start_practice(&self, topic_id: Option<&str>) -> Result<DeckView, AppError> {
        let g = self.ground(topic_id)?;
        let now = self.deps.clock.now();
        let deck = compose(&g.deck_outcomes(), g.today(now), now);
        let mut cards = vec![];
        for card in &deck {
            let item =
                self.store.item(&card.item_id)?.ok_or_else(|| AppError::Refused("a question went missing".into()))?;
            let opts = options(&item)?;
            let order = shuffled_order(opts.len(), self.deps.rng.as_ref());
            let (outcome, concept) = g.concept(&card.concept_id).expect("a card's concept is in its ground");
            let topic =
                g.topics.iter().find(|t| t.id == outcome.topic_id).expect("an outcome's topic is in its ground");
            cards.push(CardView {
                item_id: item.id.clone(),
                reason: match card.reason {
                    Reason::Due => "due",
                    Reason::New => "new",
                }
                .into(),
                stem: item.stem.clone(),
                options: order.iter().map(|&i| opts[i].text.clone()).collect(),
                order: order.iter().map(|&i| i as u32).collect(),
                topic_title: topic.title.clone(),
                outcome_ord: outcome.ord,
                concept: concept.statement.clone(),
            });
        }
        let practised: HashSet<&str> = g.attempts.iter().map(|a| a.topic_id.as_str()).collect();
        let first_set = !cards.is_empty()
            && deck.iter().all(|c| {
                c.reason == Reason::New
                    && g.concept(&c.concept_id).is_some_and(|(o, _)| !practised.contains(o.topic_id.as_str()))
            });
        Ok(DeckView { topic_id: topic_id.map(String::from), cards, first_set })
    }

    pub(crate) fn answer(&self, item_id: &str, order: &[u32], chosen: u32) -> Result<AnswerView, AppError> {
        let item = self.store.item(item_id)?.ok_or_else(|| AppError::Refused("there's no such question".into()))?;
        if item.status != "active" {
            return Err(AppError::Refused("that question isn't in use any more".into()));
        }
        let opts = options(&item)?;
        // The order shown must be the stored options, each once: grading
        // never trusts the UI's idea of which letter was right.
        let mut seen: Vec<u32> = order.to_vec();
        seen.sort_unstable();
        if seen != (0..opts.len() as u32).collect::<Vec<_>>() || chosen as usize >= opts.len() {
            return Err(AppError::Refused("that answer doesn't match the question's options".into()));
        }
        let picked = order[chosen as usize] as usize;
        let correct = opts[picked].correct;

        let before = self.ground(Some(&item.topic_id))?.standings();
        let attempt = AttemptRow {
            id: self.id(),
            topic_id: item.topic_id.clone(),
            outcome_id: item.outcome_id.clone(),
            item_id: item.id.clone(),
            order: order.iter().map(|&i| i as usize).collect(),
            chosen: picked,
            correct,
            at: self.now(),
            disputed_at: None,
        };
        self.store.insert_attempt(&attempt)?;
        let g = self.ground(Some(&item.topic_id))?;
        let after = g.standings();
        let name = |s: Option<&Standing>| s.map(|s| s.state).unwrap_or(ConceptState::Unseen).name().to_string();
        let concepts = item
            .concept_ids
            .iter()
            .filter_map(|c| {
                g.concept(c).map(|(_, row)| ConceptChange {
                    concept: row.statement.clone(),
                    before: name(before.get(c)),
                    after: name(after.get(c)),
                })
            })
            .collect();
        for c in &item.concept_ids {
            self.find_if_missed(c, after.get(c).map(|s| s.state).unwrap_or(ConceptState::Unseen));
        }
        self.changed(&item.topic_id);
        Ok(AnswerView {
            attempt_id: attempt.id,
            correct,
            picked: chosen,
            key: order.iter().position(|&i| opts[i as usize].correct).unwrap_or(0) as u32,
            options: order
                .iter()
                .map(|&i| {
                    let o = &opts[i as usize];
                    OptionFeedback {
                        text: o.text.clone(),
                        correct: o.correct,
                        why: o.why.clone(),
                        misconception: o.misconception.clone(),
                    }
                })
                .collect(),
            source: item.source_ref.clone(),
            model_knowledge: item.model_knowledge,
            concepts,
        })
    }

    /// "This key is wrong": the answer is voided and its question retired.
    pub(crate) fn dispute(&self, attempt_id: &str) -> Result<DisputeView, AppError> {
        let attempt = self
            .store
            .attempts(None)?
            .into_iter()
            .find(|a| a.id == attempt_id)
            .ok_or_else(|| AppError::Refused("there's no such answer".into()))?;
        let before = self.ground(Some(&attempt.topic_id))?.standings();
        self.store.dispute(attempt_id, &self.now())?;
        let g = self.ground(Some(&attempt.topic_id))?;
        let after = g.standings();
        let name = |s: Option<&Standing>| s.map(|s| s.state).unwrap_or(ConceptState::Unseen).name().to_string();
        let concepts = g
            .item_concepts
            .get(&attempt.item_id)
            .cloned()
            .or_else(|| self.store.item(&attempt.item_id).ok().flatten().map(|i| i.concept_ids))
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                g.concept(c).map(|(_, row)| ConceptChange {
                    concept: row.statement.clone(),
                    before: name(before.get(c)),
                    after: name(after.get(c)),
                })
            })
            .collect();
        self.changed(&attempt.topic_id);
        Ok(DisputeView { attempt_id: attempt_id.into(), concepts })
    }

    pub(crate) fn summary(&self, attempt_ids: &[String]) -> Result<SummaryView, AppError> {
        let all = self.store.attempts(None)?;
        // Disputed answers don't count, here or anywhere.
        let set: Vec<&AttemptRow> =
            all.iter().filter(|a| attempt_ids.contains(&a.id) && a.disputed_at.is_none()).collect();
        let topics: Vec<String> = set.iter().map(|a| a.topic_id.clone()).collect::<HashSet<_>>().into_iter().collect();
        let mut held = vec![];
        let mut slipped = vec![];
        let mut outcomes = vec![];
        let mut next_due: Option<DateTime<Utc>> = None;
        let mut first_set = !set.is_empty();
        for t in &topics {
            let g = self.ground(Some(t))?;
            let standings = g.standings();
            let answers = g.answers();
            // A starting point: every answer in the set was its concept's first.
            for a in set.iter().filter(|a| &a.topic_id == t) {
                for c in g.item_concepts.get(&a.item_id).into_iter().flatten() {
                    let earlier = answers.get(c).into_iter().flatten().filter(|x| x.item_id != a.item_id);
                    let at = parse_sql_time(&a.at);
                    if earlier.into_iter().any(|x| Some(x.at) < at) {
                        first_set = false;
                    }
                }
            }
            // A concept's last answer in this set decides held or slipped.
            let mut last: Vec<(String, bool)> = vec![];
            for a in set.iter().filter(|a| &a.topic_id == t) {
                for c in g.item_concepts.get(&a.item_id).into_iter().flatten() {
                    last.retain(|(id, _)| id != c);
                    last.push((c.clone(), a.correct));
                }
            }
            for (c, right) in &last {
                let Some((_, row)) = g.concept(c) else { continue };
                let s = standings.get(c).cloned().unwrap_or_default();
                next_due = match (next_due, s.due) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                let line = ConceptLine {
                    concept_id: c.clone(),
                    concept: row.statement.clone(),
                    studied: self.store.studied_at(c)?.is_some(),
                    state: s.state.name().into(),
                };
                if *right { held.push(line) } else { slipped.push(line) }
            }
            let touched: HashSet<&str> =
                set.iter().filter(|a| &a.topic_id == t).map(|a| a.outcome_id.as_str()).collect();
            for (o, concepts, _) in g.outcomes.iter().filter(|(o, _, _)| touched.contains(o.id.as_str())) {
                outcomes.push(g.standing_of(o, concepts, &standings));
            }
        }
        Ok(SummaryView {
            answered: set.len() as i64,
            correct: set.iter().filter(|a| a.correct).count() as i64,
            held,
            slipped,
            next_due: next_due.map(sql_time),
            next_due_in_days: next_due.map(|d| days_until(d, self.deps.clock.now())),
            first_set,
            outcomes,
        })
    }

    pub(crate) fn progress(&self, topic_id: &str) -> Result<ProgressView, AppError> {
        let g = self.ground(Some(topic_id))?;
        let answers = g.answers();
        let standings = g.standings();
        let outcomes = g
            .outcomes
            .iter()
            .map(|(o, concepts, _)| OutcomeProgress {
                standing: g.standing_of(o, concepts, &standings),
                concepts: concepts
                    .iter()
                    .map(|c| {
                        let s = standings.get(&c.id).cloned().unwrap_or_default();
                        ConceptProgress {
                            concept_id: c.id.clone(),
                            statement: c.statement.clone(),
                            state: s.state.name().into(),
                            due: s.due.map(sql_time),
                            answers: answers.get(&c.id).map(|a| a.len()).unwrap_or(0) as i64,
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(ProgressView { topic_id: topic_id.into(), outcomes, attempts: g.attempts.len() as i64 })
    }

    /// Every concept's standing in a topic, by concept id.
    pub(crate) fn standings(&self, topic_id: &str) -> Result<HashMap<String, Standing>, AppError> {
        Ok(self.ground(Some(topic_id))?.standings())
    }

    /// One concept's standing, from its topic's answers.
    pub(crate) fn standing_of_concept(&self, topic_id: &str, concept_id: &str) -> Result<Standing, AppError> {
        Ok(self.ground(Some(topic_id))?.standings().remove(concept_id).unwrap_or_default())
    }

    /// What a topic's next set would hold, whether it's been practised, and
    /// (when there's nothing to practise) the days until its next recall.
    pub(crate) fn practice_state(&self, topic_id: &str) -> Result<PracticeState, AppError> {
        let g = self.ground(Some(topic_id))?;
        let now = self.deps.clock.now();
        let deck = compose(&g.deck_outcomes(), g.today(now), now);
        let next = if deck.is_empty() { g.standings().values().filter_map(|s| s.due).min() } else { None };
        let new_outcome = deck.iter().find(|c| c.reason == Reason::New).map(|c| c.outcome_id.clone());
        Ok(PracticeState {
            due: deck.iter().filter(|c| c.reason == Reason::Due).count() as i64,
            new: deck.iter().filter(|c| c.reason == Reason::New).count() as i64,
            new_outcome_ord: new_outcome
                .and_then(|id| g.outcomes.iter().find(|(o, _, _)| o.id == id).map(|(o, _, _)| o.ord)),
            practised: !g.attempts.is_empty(),
            next_due_in_days: next.map(|d| days_until(d, now)),
        })
    }

    /// Today's one next step, across topics.
    pub(crate) fn next_step(&self) -> Result<NextStep, AppError> {
        let g = self.ground(None)?;
        let all = self.store.all_topics()?;
        if all.is_empty() {
            return Ok(NextStep::StartTopic);
        }
        if g.topics.is_empty()
            && let Some(t) = all.iter().find(|t| t.status == "draft" && t.origin == "learner")
        {
            return Ok(NextStep::FinishSetup { topic_id: t.id.clone(), title: t.title.clone() });
        }
        let now = self.deps.clock.now();
        let deck = compose(&g.deck_outcomes(), g.today(now), now);
        let practised: HashSet<&str> = g.attempts.iter().map(|a| a.topic_id.as_str()).collect();
        if !deck.is_empty() {
            let first_set = deck
                .iter()
                .filter(|c| c.reason == Reason::New)
                .filter_map(|c| g.concept(&c.concept_id))
                .map(|(o, _)| o.topic_id.clone())
                .find(|t| !practised.contains(t.as_str()));
            let mut topics: Vec<String> = vec![];
            for c in &deck {
                if let Some((o, _)) = g.concept(&c.concept_id)
                    && let Some(t) = g.topics.iter().find(|t| t.id == o.topic_id)
                    && !topics.contains(&t.title)
                {
                    topics.push(t.title.clone());
                }
            }
            return Ok(NextStep::Practise {
                due: deck.iter().filter(|c| c.reason == Reason::Due).count() as i64,
                new: deck.iter().filter(|c| c.reason == Reason::New).count() as i64,
                first_set,
                topics,
            });
        }
        let standings = g.standings();
        let next = standings.values().filter_map(|s| s.due).filter(|d| *d > now).min();
        // Missed and not read up on yet, in outcome order.
        let unread: Vec<&str> = g
            .outcomes
            .iter()
            .flat_map(|(_, cs, _)| cs.iter())
            .filter(|c| standings.get(&c.id).is_some_and(|s| s.state == ConceptState::Seen))
            .map(|c| c.id.as_str())
            .filter(|c| self.store.studied_at(c).ok().flatten().is_none())
            .collect();
        if let Some(first) = unread.first() {
            return Ok(NextStep::Learn {
                count: unread.len() as i64,
                concept_id: first.to_string(),
                next_due_in_days: next.map(|d| days_until(d, now)),
            });
        }
        // Questions are being written for a topic not practised yet (one with
        // outcomes: a topic only on the map has nothing to write).
        let has_outcomes = |id: &str| g.outcomes.iter().any(|(o, _, _)| o.topic_id == id);
        if let Some(t) = g.topics.iter().find(|t| !practised.contains(t.id.as_str()) && has_outcomes(&t.id)) {
            return Ok(NextStep::Preparing { topic_id: t.id.clone(), title: t.title.clone() });
        }
        Ok(NextStep::NothingDue { next_at: next.map(sql_time), next_due_in_days: next.map(|d| days_until(d, now)) })
    }

    /// This week on Today, Monday to Sunday in the learner's time: the days
    /// they practised; and concepts by state across topics.
    pub(crate) fn week(&self) -> Result<(String, Vec<DayView>, StateCounts), AppError> {
        let g = self.ground(None)?;
        let day = |t: DateTime<Utc>| t.with_timezone(&self.deps.utc_offset).date_naive();
        let today = day(self.deps.clock.now());
        let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
        let practised: HashSet<chrono::NaiveDate> =
            g.attempts.iter().filter_map(|a| parse_sql_time(&a.at)).map(day).collect();
        let week = (0..7)
            .map(|i| {
                let d = monday + chrono::Duration::days(i);
                DayView { date: d.to_string(), practised: practised.contains(&d), today: d == today, future: d > today }
            })
            .collect();
        let standings = g.standings();
        let mut counts = StateCounts::default();
        for (_, cs, _) in &g.outcomes {
            for c in cs {
                match standings.get(&c.id).map(|s| s.state).unwrap_or(ConceptState::Unseen) {
                    ConceptState::Retained => counts.retained += 1,
                    ConceptState::Recalled => counts.recalled += 1,
                    ConceptState::Seen => counts.to_learn += 1,
                    ConceptState::Unseen => counts.unseen += 1,
                }
            }
        }
        Ok((today.to_string(), week, counts))
    }
}
