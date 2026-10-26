//! Revised Bloom (Anderson & Krathwohl): the tier classifies the task an
//! item sets, fixed when the item is written. Learners see plain words, not
//! the taxonomy's names.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Remember,
    Understand,
    Apply,
    Analyze,
    Evaluate,
    Create,
}

impl Tier {
    pub const ALL: [Tier; 6] =
        [Tier::Remember, Tier::Understand, Tier::Apply, Tier::Analyze, Tier::Evaluate, Tier::Create];

    pub fn name(self) -> &'static str {
        match self {
            Tier::Remember => "remember",
            Tier::Understand => "understand",
            Tier::Apply => "apply",
            Tier::Analyze => "analyze",
            Tier::Evaluate => "evaluate",
            Tier::Create => "create",
        }
    }

    pub fn parse(s: &str) -> Option<Tier> {
        let s = s.trim().to_lowercase();
        let s = if s == "analyse" { "analyze".to_string() } else { s };
        Tier::ALL.into_iter().find(|t| t.name() == s)
    }

    /// What the learner reads: "Target: judge and recommend".
    pub fn plain(self) -> &'static str {
        match self {
            Tier::Remember => "recall it",
            Tier::Understand => "explain it",
            Tier::Apply => "use it",
            Tier::Analyze => "work out why",
            Tier::Evaluate => "judge and recommend",
            Tier::Create => "design it",
        }
    }

    pub fn from_plain(s: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.plain() == s.trim())
    }

    /// Recall items test remember and understand.
    pub fn is_recall(self) -> bool {
        matches!(self, Tier::Remember | Tier::Understand)
    }

    /// The tier apply items are written at: apply, or the target if it's
    /// lower (an outcome aimed at understanding is checked at understanding).
    pub fn apply_tier_for(target: Tier) -> Tier {
        target.min(Tier::Apply)
    }

    /// Whether an outcome needs target items of its own: only when its target
    /// is above its apply tier.
    pub fn needs_target_items(target: Tier) -> bool {
        target > Tier::apply_tier_for(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_order_and_read_plainly() {
        assert!(Tier::Evaluate > Tier::Apply);
        assert_eq!(Tier::apply_tier_for(Tier::Evaluate), Tier::Apply);
        assert_eq!(Tier::apply_tier_for(Tier::Understand), Tier::Understand);
        assert!(Tier::needs_target_items(Tier::Evaluate) && !Tier::needs_target_items(Tier::Apply));
        assert_eq!(Tier::from_plain("judge and recommend"), Some(Tier::Evaluate));
        assert_eq!(Tier::parse(" Analyse "), Some(Tier::Analyze));
    }
}
