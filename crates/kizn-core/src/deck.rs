//! The practice set, built in code each time practice starts (PRD §6.7). The
//! model never sees it. This phase's slots, in order:
//!
//! 1. Due recalls, oldest first, at most DAILY_RECALL_CAP a day.
//! 2. Placement: one recall question per concept never answered on the
//!    outcome being worked (the first, in order, with one), up to MAX_NEW.
//!
//! Apply, target, explain and judge cards arrive with later phases, in their
//! places after these. Question choice per concept: never shown first, else
//! the one shown longest ago, never one shown in the last RETAIN_GAP_DAYS (so
//! "retained" always rests on a different question).

use chrono::{DateTime, Duration, Utc};

use crate::progress::{ConceptState, RETAIN_GAP_DAYS, Standing};

pub const MAX_CARDS: usize = 12;
pub const MAX_NEW: usize = 5;
pub const DAILY_RECALL_CAP: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A recall that's come due.
    Due,
    /// Placement: the first question on a concept.
    New,
}

/// A recall question that could go in the set, and when it was last shown.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub item_id: String,
    pub last_shown: Option<DateTime<Utc>>,
}

/// A concept, its standing, and its recall questions.
#[derive(Debug, Clone)]
pub struct DeckConcept {
    pub id: String,
    pub standing: Standing,
    pub recall: Vec<Candidate>,
}

/// An outcome's concepts, in the order the outcomes are worked.
#[derive(Debug, Clone)]
pub struct DeckOutcome {
    pub id: String,
    pub concepts: Vec<DeckConcept>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckCard {
    pub item_id: String,
    pub concept_id: String,
    pub outcome_id: String,
    pub reason: Reason,
}

/// Never shown first, else the one shown longest ago; never one shown in the
/// last RETAIN_GAP_DAYS or already chosen.
pub fn pick<'a>(candidates: &'a [Candidate], now: DateTime<Utc>, used: &[String]) -> Option<&'a Candidate> {
    let ok: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| !used.contains(&c.item_id))
        .filter(|c| c.last_shown.is_none_or(|t| now - t >= Duration::days(RETAIN_GAP_DAYS)))
        .collect();
    ok.iter().find(|c| c.last_shown.is_none()).copied().or_else(|| ok.iter().min_by_key(|c| c.last_shown).copied())
}

/// The set: due recalls, then placement on the outcome being worked.
/// `recalls_today` is how many recall answers were already given today.
pub fn compose(outcomes: &[DeckOutcome], recalls_today: usize, now: DateTime<Utc>) -> Vec<DeckCard> {
    let mut cards: Vec<DeckCard> = vec![];
    let mut used: Vec<String> = vec![];

    // 1. Due recalls, oldest first, within the day's cap.
    let mut due: Vec<(&DeckOutcome, &DeckConcept)> = outcomes
        .iter()
        .flat_map(|o| o.concepts.iter().map(move |c| (o, c)))
        .filter(|(_, c)| c.standing.state != ConceptState::Unseen && c.standing.due.is_some_and(|d| d <= now))
        .collect();
    due.sort_by_key(|(_, c)| c.standing.due);
    let mut room = DAILY_RECALL_CAP.saturating_sub(recalls_today);
    for (o, c) in due {
        if room == 0 || cards.len() >= MAX_CARDS {
            break;
        }
        if let Some(item) = pick(&c.recall, now, &used) {
            used.push(item.item_id.clone());
            cards.push(DeckCard {
                item_id: item.item_id.clone(),
                concept_id: c.id.clone(),
                outcome_id: o.id.clone(),
                reason: Reason::Due,
            });
            room -= 1;
        }
    }

    // 2. Placement: the first outcome with a concept never answered that has
    //    a question to ask, one question per such concept.
    let working = outcomes.iter().find(|o| {
        o.concepts.iter().any(|c| c.standing.state == ConceptState::Unseen && pick(&c.recall, now, &used).is_some())
    });
    if let Some(o) = working {
        let mut fresh = 0;
        for c in o.concepts.iter().filter(|c| c.standing.state == ConceptState::Unseen) {
            if fresh >= MAX_NEW || cards.len() >= MAX_CARDS {
                break;
            }
            if let Some(item) = pick(&c.recall, now, &used) {
                used.push(item.item_id.clone());
                cards.push(DeckCard {
                    item_id: item.item_id.clone(),
                    concept_id: c.id.clone(),
                    outcome_id: o.id.clone(),
                    reason: Reason::New,
                });
                fresh += 1;
            }
        }
    }
    cards
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::progress::{Answer, standing};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 10, 9, 0, 0).unwrap()
    }

    fn concept(id: &str, answers: &[Answer], items: &[&str]) -> DeckConcept {
        DeckConcept {
            id: id.into(),
            standing: standing(answers),
            recall: items.iter().map(|i| Candidate { item_id: i.to_string(), last_shown: None }).collect(),
        }
    }

    #[test]
    fn placement_is_one_question_per_new_concept_up_to_five() {
        let o = DeckOutcome {
            id: "o1".into(),
            concepts: (0..7).map(|i| concept(&format!("c{i}"), &[], &[&format!("q{i}a"), &format!("q{i}b")])).collect(),
        };
        let cards = compose(&[o], 0, now());
        assert_eq!(cards.len(), MAX_NEW);
        assert!(cards.iter().all(|c| c.reason == Reason::New));
        let concepts: std::collections::HashSet<_> = cards.iter().map(|c| &c.concept_id).collect();
        assert_eq!(concepts.len(), MAX_NEW, "one per concept");
    }

    #[test]
    fn due_recalls_come_first_on_a_different_question() {
        let answered = Answer { item_id: "q0a".into(), correct: true, at: now() - Duration::days(4) };
        let o = DeckOutcome {
            id: "o1".into(),
            concepts: vec![concept("c0", &[answered], &["q0a", "q0b"]), concept("c1", &[], &["q1a"])],
        };
        let mut o = o;
        o.concepts[0].recall[0].last_shown = Some(now() - Duration::days(4));
        let cards = compose(&[o], 0, now());
        assert_eq!(
            cards[0],
            DeckCard { item_id: "q0b".into(), concept_id: "c0".into(), outcome_id: "o1".into(), reason: Reason::Due }
        );
        assert_eq!(cards[1].reason, Reason::New);
    }

    #[test]
    fn the_daily_cap_holds_due_recalls_back() {
        let due = |i: usize| Answer { item_id: format!("q{i}"), correct: true, at: now() - Duration::days(4) };
        let o = DeckOutcome {
            id: "o1".into(),
            concepts: (0..14)
                .map(|i| concept(&format!("c{i}"), &[due(i)], &[&format!("q{i}"), &format!("r{i}")]))
                .collect(),
        };
        assert_eq!(compose(std::slice::from_ref(&o), 0, now()).len(), DAILY_RECALL_CAP);
        assert_eq!(compose(&[o], 8, now()).len(), 2);
    }

    #[test]
    fn a_question_shown_recently_is_not_shown_again() {
        let recent = Candidate { item_id: "q".into(), last_shown: Some(now() - Duration::days(1)) };
        assert!(pick(&[recent], now(), &[]).is_none());
    }
}
