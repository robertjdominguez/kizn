//! What a learner knows, worked out from their answers and never stored:
//! each concept's state, and when it should come back. Pure: replay a
//! concept's answers in order and the standing falls out.
//!
//! States (PRD §6.5): not started (no answer) → seen (answered, not recalled
//! now) → recalled (right once) → retained (right again, on a different
//! question, at least 3 days after the answer it's measured from). A miss
//! drops a concept one state and brings it back the next day. The first
//! answer on a concept is placement: right means recalled.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// A second correct this long after the first makes a concept retained; also
/// the first recall interval.
pub const RETAIN_GAP_DAYS: i64 = 3;
/// A miss brings the concept back the next day.
pub const MISS_INTERVAL_DAYS: i64 = 1;
/// Retained intervals grow by this, capped.
pub const GROWTH: f64 = 2.5;
pub const MAX_INTERVAL_DAYS: f64 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConceptState {
    Unseen,
    Seen,
    Recalled,
    Retained,
}

impl ConceptState {
    pub fn name(self) -> &'static str {
        match self {
            ConceptState::Unseen => "unseen",
            ConceptState::Seen => "seen",
            ConceptState::Recalled => "recalled",
            ConceptState::Retained => "retained",
        }
    }

    /// What the learner reads.
    pub fn plain(self) -> &'static str {
        match self {
            ConceptState::Unseen => "not started",
            ConceptState::Seen => "seen",
            ConceptState::Recalled => "recalled",
            ConceptState::Retained => "stuck",
        }
    }
}

/// One answer that counts for a concept: an attempt on an item that names it
/// (disputed attempts are left out by the caller).
#[derive(Debug, Clone)]
pub struct Answer {
    pub item_id: String,
    pub correct: bool,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Standing {
    pub state: ConceptState,
    pub correct: u32,
    pub misses: u32,
    /// The correct answer the next one is measured from (time and item).
    pub anchor: Option<(DateTime<Utc>, String)>,
    /// The last recall interval, for growth.
    pub interval_days: Option<f64>,
    /// When it should come back; none until answered.
    pub due: Option<DateTime<Utc>>,
    pub last: Option<DateTime<Utc>>,
}

impl Default for Standing {
    fn default() -> Self {
        Standing {
            state: ConceptState::Unseen,
            correct: 0,
            misses: 0,
            anchor: None,
            interval_days: None,
            due: None,
            last: None,
        }
    }
}

fn days(d: f64) -> Duration {
    Duration::seconds((d * 86_400.0).round() as i64)
}

/// A concept's standing from its answers, oldest first.
pub fn standing(answers: &[Answer]) -> Standing {
    let mut s = Standing::default();
    for a in answers {
        s.last = Some(a.at);
        if !a.correct {
            s.misses += 1;
            s.state = match s.state {
                ConceptState::Retained => ConceptState::Recalled,
                _ => {
                    s.anchor = None;
                    ConceptState::Seen
                }
            };
            s.interval_days = Some(MISS_INTERVAL_DAYS as f64);
            s.due = Some(a.at + Duration::days(MISS_INTERVAL_DAYS));
            continue;
        }
        s.correct += 1;
        match s.state {
            ConceptState::Retained => {
                let next = s.interval_days.map(|d| (d * GROWTH).clamp(RETAIN_GAP_DAYS as f64, MAX_INTERVAL_DAYS));
                s.interval_days = Some(next.unwrap_or(RETAIN_GAP_DAYS as f64));
                s.anchor = Some((a.at, a.item_id.clone()));
            }
            ConceptState::Recalled => {
                let stuck = s
                    .anchor
                    .as_ref()
                    .is_some_and(|(at, item)| a.at - *at >= Duration::days(RETAIN_GAP_DAYS) && *item != a.item_id);
                if stuck {
                    s.state = ConceptState::Retained;
                    s.anchor = Some((a.at, a.item_id.clone()));
                    s.interval_days =
                        Some((RETAIN_GAP_DAYS as f64 * GROWTH).clamp(RETAIN_GAP_DAYS as f64, MAX_INTERVAL_DAYS));
                } else {
                    // A second correct the same day (or on the same item)
                    // doesn't move the clock.
                    s.interval_days = Some(RETAIN_GAP_DAYS as f64);
                }
            }
            ConceptState::Unseen | ConceptState::Seen => {
                s.state = ConceptState::Recalled;
                s.anchor = Some((a.at, a.item_id.clone()));
                s.interval_days = Some(RETAIN_GAP_DAYS as f64);
            }
        }
        let interval = s.interval_days.unwrap_or(RETAIN_GAP_DAYS as f64);
        let from = s.anchor.as_ref().map(|(at, _)| *at).unwrap_or(a.at);
        s.due = Some(from + days(interval));
    }
    s
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
    }

    fn a(item: &str, correct: bool, day: u32, hour: u32) -> Answer {
        Answer { item_id: item.into(), correct, at: at(day, hour) }
    }

    #[test]
    fn placement_right_is_recalled_and_comes_back_in_three_days() {
        let s = standing(&[a("i1", true, 1, 9)]);
        assert_eq!(s.state, ConceptState::Recalled);
        assert_eq!(s.due, Some(at(4, 9)));
    }

    #[test]
    fn a_miss_is_seen_and_comes_back_tomorrow() {
        let s = standing(&[a("i1", false, 1, 9)]);
        assert_eq!(s.state, ConceptState::Seen);
        assert_eq!(s.due, Some(at(2, 9)));
    }

    #[test]
    fn retained_needs_a_different_question_three_days_later() {
        assert_eq!(standing(&[a("i1", true, 1, 9), a("i2", true, 1, 15)]).state, ConceptState::Recalled);
        assert_eq!(standing(&[a("i1", true, 1, 9), a("i1", true, 4, 9)]).state, ConceptState::Recalled);
        let s = standing(&[a("i1", true, 1, 9), a("i2", true, 4, 10)]);
        assert_eq!(s.state, ConceptState::Retained);
        assert!(s.due.unwrap() > at(11, 0), "retained intervals grow: {:?}", s.due);
    }

    #[test]
    fn a_miss_drops_one_state() {
        let s = standing(&[a("i1", true, 1, 9), a("i2", true, 4, 10), a("i3", false, 12, 9)]);
        assert_eq!(s.state, ConceptState::Recalled);
        let s = standing(&[a("i1", true, 1, 9), a("i2", false, 2, 9)]);
        assert_eq!(s.state, ConceptState::Seen);
        assert_eq!(s.due, Some(at(3, 9)));
    }
}
