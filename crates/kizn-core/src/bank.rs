//! What makes a written item fit to show, checked in code before it can go
//! live, and what an outcome's bank should hold. Pure functions over what the
//! model wrote and what the store holds.
//!
//! The checks are the item-writing guidelines that can be checked mechanically
//! (Haladyna, Downing & Rodriguez 2002): one best answer, no "all/none of the
//! above", no longest-option cue, no answer given away in the stem, plausible
//! distractors that each stand for a misconception.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Tier;

// ---- what the bank holds -------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    /// One concept, remember or understand.
    Recall,
    /// A scenario drawing on two or more concepts, at the apply tier.
    Apply,
    /// A scenario at the outcome's target tier.
    Target,
    /// Unscored: a prompt to explain, with an exemplar.
    Explain,
}

impl ItemKind {
    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Recall => "recall",
            ItemKind::Apply => "apply",
            ItemKind::Target => "target",
            ItemKind::Explain => "explain",
        }
    }

    pub fn parse(s: &str) -> Option<ItemKind> {
        [ItemKind::Recall, ItemKind::Apply, ItemKind::Target, ItemKind::Explain].into_iter().find(|k| k.name() == s)
    }

    /// What a full slot of this kind holds.
    pub fn slot_size(self) -> usize {
        match self {
            ItemKind::Recall => 4,
            ItemKind::Apply => 4,
            ItemKind::Target => 4,
            ItemKind::Explain => 2,
        }
    }
}

/// A slot is refilled when it has fewer unseen active items than this.
pub const REFILL_BELOW: usize = 2;
pub const MIN_OPTIONS: usize = 3;
pub const MAX_OPTIONS: usize = 5;
pub const MIN_CONCEPTS: usize = 3;
pub const MAX_CONCEPTS: usize = 7;
pub const MIN_OUTCOMES: usize = 3;
pub const MAX_OUTCOMES: usize = 5;

/// A recall slot belongs to one concept; the others to the outcome.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Slot {
    pub kind: ItemKind,
    pub concept_id: Option<String>,
}

/// The slots an outcome has: recall per concept, apply, target (if its
/// target is above apply), explain.
pub fn slots_for(target: Tier, concept_ids: &[String]) -> Vec<Slot> {
    let mut slots: Vec<Slot> =
        concept_ids.iter().map(|id| Slot { kind: ItemKind::Recall, concept_id: Some(id.clone()) }).collect();
    slots.push(Slot { kind: ItemKind::Apply, concept_id: None });
    if Tier::needs_target_items(target) {
        slots.push(Slot { kind: ItemKind::Target, concept_id: None });
    }
    slots.push(Slot { kind: ItemKind::Explain, concept_id: None });
    slots
}

/// An item already in the bank, as far as filling slots goes.
pub struct BankedItem<'a> {
    pub id: &'a str,
    pub kind: ItemKind,
    pub concept_ids: &'a [String],
}

/// The slots to write for, and how many. A slot is written up to its full
/// size until it has held that many active items; after that, once practice
/// has used its items up (fewer than REFILL_BELOW unseen), it's topped back
/// up. Either way the answer depends only on what's stored, so a restart
/// mid-write carries on where it was.
pub fn bank_needs(
    target: Tier,
    concept_ids: &[String],
    active: &[BankedItem],
    attempted: &HashSet<String>,
) -> Vec<(Slot, usize)> {
    slots_for(target, concept_ids)
        .into_iter()
        .filter_map(|slot| {
            let in_slot: Vec<&BankedItem> = active
                .iter()
                .filter(|i| {
                    i.kind == slot.kind && slot.concept_id.as_ref().is_none_or(|c| i.concept_ids.first() == Some(c))
                })
                .collect();
            let fresh = in_slot.iter().filter(|i| !attempted.contains(i.id)).count();
            let size = slot.kind.slot_size();
            let short = size.saturating_sub(in_slot.len());
            let refill = if fresh < REFILL_BELOW { size - fresh } else { 0 };
            let write = short.max(refill);
            (write > 0).then(|| (slot.clone(), write))
        })
        .collect()
}

/// One outcome's bank, as far as deciding what to write next goes.
pub struct OutcomeBank<'a> {
    pub target: Tier,
    pub concept_ids: &'a [String],
    pub active: Vec<BankedItem<'a>>,
    pub attempted: &'a HashSet<String>,
}

/// What to write next across a topic's outcomes (in their order), in the
/// order the learner will need it: outcome 1's placement (one recall per
/// concept: the first set), a second recall on each of its concepts (Day 3
/// needs a different question), the other outcomes' placement and second
/// recall, then every recall slot to full, then "use it", target and explain
/// questions. Returns the outcome's index, the slot and how many to write.
pub fn next_need(outcomes: &[OutcomeBank]) -> Option<(usize, Slot, usize)> {
    ranked_need(outcomes).map(|(_, i, slot, n)| (i, slot, n))
}

/// Like `next_need`, with how soon the learner needs it: 0 is outcome 1's
/// first set, 1 its second recall, 2 and 3 the other outcomes' first set and
/// second recall, 4 to 7 the rest by kind (recall, use it, target, explain).
pub fn ranked_need(outcomes: &[OutcomeBank]) -> Option<(u8, usize, Slot, usize)> {
    let in_slot = |o: &OutcomeBank, concept: &String| {
        o.active.iter().filter(|i| i.kind == ItemKind::Recall && i.concept_ids.first() == Some(concept)).count()
    };
    // Recall up to a level: first outcome 1, then the rest.
    let order: Vec<usize> = (0..outcomes.len()).collect();
    let passes: [(&[usize], usize); 4] = [
        (&order[..order.len().min(1)], 1),
        (&order[..order.len().min(1)], 2),
        (&order[order.len().min(1)..], 1),
        (&order[order.len().min(1)..], 2),
    ];
    for (rank, (which, level)) in passes.into_iter().enumerate() {
        for &i in which {
            let o = &outcomes[i];
            for c in o.concept_ids {
                let have = in_slot(o, c);
                if have < level {
                    let slot = Slot { kind: ItemKind::Recall, concept_id: Some(c.clone()) };
                    return Some((rank as u8, i, slot, level - have));
                }
            }
        }
    }
    // Then by kind, outcome by outcome: the rest of recall, use it, target, explain.
    for (k, kind) in [ItemKind::Recall, ItemKind::Apply, ItemKind::Target, ItemKind::Explain].into_iter().enumerate() {
        for (i, o) in outcomes.iter().enumerate() {
            if let Some((slot, n)) =
                bank_needs(o.target, o.concept_ids, &o.active, o.attempted).into_iter().find(|(s, _)| s.kind == kind)
            {
                return Some((4 + k as u8, i, slot, n));
            }
        }
    }
    None
}

/// What to write next across topics (oldest first): the need that comes
/// soonest for the learner, whichever topic it's in, so a new topic gets its
/// first set before an older one's bank is filled to the deepest kinds. The
/// oldest topic wins a tie. Returns the topic's index with `next_need`'s.
pub fn next_across(topics: &[Vec<OutcomeBank>]) -> Option<(usize, usize, Slot, usize)> {
    topics
        .iter()
        .enumerate()
        .filter_map(|(t, outcomes)| ranked_need(outcomes).map(|(rank, i, slot, n)| (rank, t, i, slot, n)))
        .min_by_key(|(rank, t, ..)| (*rank, *t))
        .map(|(_, t, i, slot, n)| (t, i, slot, n))
}

/// Whether an outcome's first set can be offered: one recall question on
/// every concept.
pub fn placement_ready(concept_ids: &[String], active: &[BankedItem]) -> bool {
    !concept_ids.is_empty()
        && concept_ids
            .iter()
            .all(|c| active.iter().any(|i| i.kind == ItemKind::Recall && i.concept_ids.first() == Some(c)))
}

/// The total an outcome's bank holds when full, for "N of ~M items ready".
pub fn bank_size(target: Tier, concepts: usize) -> usize {
    concepts * ItemKind::Recall.slot_size()
        + ItemKind::Apply.slot_size()
        + if Tier::needs_target_items(target) { ItemKind::Target.slot_size() } else { 0 }
        + ItemKind::Explain.slot_size()
}

// ---- checking one item ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foundation {
    /// A topic on the map, when the tag names one.
    pub topic_id: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemOption {
    pub text: String,
    pub correct: bool,
    pub why: String,
    pub misconception: Option<String>,
    pub foundation: Option<Foundation>,
}

/// An item that passed the code checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedItem {
    pub kind: ItemKind,
    pub tier: Tier,
    pub concept_ids: Vec<String>,
    pub stem: String,
    pub options: Option<Vec<ItemOption>>,
    pub exemplar: Option<String>,
    pub source_ref: Option<String>,
    pub model_knowledge: bool,
}

/// Why an item was refused: a short label (what the PRD calls it) and the
/// detail, both kept with the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub label: &'static str,
    pub detail: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.label, self.detail)
    }
}

pub struct Concept<'a> {
    pub id: &'a str,
    pub name: &'a str,
}

pub struct TopicRef<'a> {
    pub id: &'a str,
    pub title: &'a str,
}

pub struct CheckContext<'a> {
    pub kind: ItemKind,
    pub target: Tier,
    /// The outcome's concepts.
    pub concepts: &'a [Concept<'a>],
    /// The concept a recall slot is for.
    pub slot_concept_id: Option<&'a str>,
    /// Stems already in the bank (not refused), with their concepts, and
    /// those accepted earlier in this batch.
    pub existing: &'a [(String, Vec<String>)],
    /// The map's topics, for foundation tags, and this outcome's own topic.
    pub topics: &'a [TopicRef<'a>],
    pub topic_id: &'a str,
}

const MAX_STEM: usize = 1500;
const MAX_OPTION: usize = 300;
const MAX_EXEMPLAR: usize = 2500;
const NEAR_DUPLICATE: f64 = 0.8;
const LONGEST_CUE: f64 = 1.5;

/// Lowercase, punctuation to spaces: for comparing wording.
pub fn norm(s: &str) -> String {
    let mapped: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' }).collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokens(s: &str) -> HashSet<String> {
    norm(s).split(' ').filter(|w| w.len() > 2).map(str::to_string).collect()
}

/// Token-set overlap (Jaccard).
pub fn overlap(a: &str, b: &str) -> f64 {
    let (x, y) = (tokens(a), tokens(b));
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    x.intersection(&y).count() as f64 / x.union(&y).count() as f64
}

/// Options that only work in a fixed position, or that test test-taking.
static POSITIONAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(all|none|both|neither) of (the|these) (above|options|answers|choices)\b|\b(options?|answers?|choices?) [a-e] (and|or|&) [a-e]\b|^\(?[a-e]\)? (and|or|&) \(?[a-e]\)?$|^both [a-e]\b",
    )
    .expect("a valid regex")
});

fn text(v: Option<&Value>) -> String {
    v.and_then(|v| v.as_str()).map(|s| s.trim().to_string()).unwrap_or_default()
}

fn median(ns: &[usize]) -> f64 {
    let mut s = ns.to_vec();
    s.sort_unstable();
    let m = s.len() / 2;
    if s.len() % 2 == 1 { s[m] as f64 } else { (s[m - 1] + s[m]) as f64 / 2.0 }
}

fn refuse(label: &'static str, detail: impl Into<String>) -> Result<CheckedItem, Refusal> {
    Err(Refusal { label, detail: detail.into() })
}

/// Checks one item as the model wrote it (a JSON object).
pub fn check_item(draft: &Value, ctx: &CheckContext) -> Result<CheckedItem, Refusal> {
    let kind = ctx.kind;

    // Concepts: names or ids, resolved against the outcome's.
    let refs: Vec<String> = match draft.get("concepts") {
        Some(Value::Array(a)) => a.iter().map(|v| text(Some(v))).filter(|s| !s.is_empty()).collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => vec![],
    };
    let mut ids: Vec<String> = vec![];
    let mut unknown: Vec<String> = vec![];
    for r in &refs {
        match ctx.concepts.iter().find(|c| c.id == r || norm(c.name) == norm(r)) {
            Some(c) if !ids.iter().any(|i| i == c.id) => ids.push(c.id.to_string()),
            Some(_) => {}
            None => unknown.push(r.clone()),
        }
    }
    if !unknown.is_empty() {
        return refuse(
            "concept not on the outcome",
            format!("names concepts the outcome doesn't have: {}", unknown.join(", ")),
        );
    }
    let concept_ids = match kind {
        ItemKind::Recall => {
            let Some(slot) = ctx.slot_concept_id else {
                return refuse("no concept", "a recall slot needs its concept");
            };
            if !ids.is_empty() && (ids.len() > 1 || ids[0] != slot) {
                return refuse(
                    "concept not on the slot",
                    "a recall item tests exactly the one concept it was written for",
                );
            }
            vec![slot.to_string()]
        }
        ItemKind::Apply | ItemKind::Target => {
            if ctx.concepts.len() >= 2 && ids.len() < 2 {
                return refuse(
                    "too few concepts",
                    format!(
                        "a{} item draws on at least two of the outcome's concepts",
                        if kind == ItemKind::Apply { "n apply" } else { " target" }
                    ),
                );
            }
            if ids.is_empty() {
                return refuse("no concept", "an item names the concepts it draws on");
            }
            ids
        }
        ItemKind::Explain => {
            if ids.is_empty() {
                ctx.concepts.iter().map(|c| c.id.to_string()).collect()
            } else {
                ids
            }
        }
    };

    // Tier: inside what the kind allows.
    let tier_raw = text(draft.get("tier"));
    let tier = match kind {
        ItemKind::Explain => Tier::parse(&tier_raw).unwrap_or(ctx.target),
        _ => {
            let Some(t) = Tier::parse(&tier_raw) else {
                return refuse("tier outside the kind", format!("the tier \"{tier_raw}\" isn't a Bloom tier"));
            };
            let want = match kind {
                ItemKind::Recall => None,
                ItemKind::Apply => Some(Tier::apply_tier_for(ctx.target)),
                _ => Some(ctx.target),
            };
            match want {
                None if !t.is_recall() => {
                    return refuse(
                        "tier outside the kind",
                        format!("a recall item is remember or understand, not {}", t.name()),
                    );
                }
                Some(w) if t != w => {
                    return refuse(
                        "tier outside the kind",
                        format!("a {} item here is at {}, not {}", kind.name(), w.name(), t.name()),
                    );
                }
                _ => t,
            }
        }
    };

    // Stem.
    let stem = text(draft.get("stem"));
    if stem.is_empty() {
        return refuse("no stem", "the item has no stem");
    }
    if stem.chars().count() > MAX_STEM {
        return refuse("too long", format!("the stem is over {MAX_STEM} characters"));
    }
    for (other, other_concepts) in ctx.existing {
        if other_concepts.iter().any(|c| concept_ids.contains(c)) && overlap(&stem, other) > NEAR_DUPLICATE {
            return refuse("near-duplicate stem", "too close to an item already in the bank");
        }
    }

    let source = text(draft.get("source"));
    let model_knowledge = draft.get("model_knowledge").and_then(|v| v.as_bool()).unwrap_or(false) || source.is_empty();
    let source_ref = (!source.is_empty()).then_some(source);

    if kind == ItemKind::Explain {
        let exemplar = text(draft.get("exemplar"));
        if exemplar.is_empty() {
            return refuse("no exemplar", "an explain item needs an exemplar: a strong answer to show after theirs");
        }
        if exemplar.chars().count() > MAX_EXEMPLAR {
            return refuse("too long", format!("the exemplar is over {MAX_EXEMPLAR} characters"));
        }
        return Ok(CheckedItem {
            kind,
            tier,
            concept_ids,
            stem,
            options: None,
            exemplar: Some(exemplar),
            source_ref,
            model_knowledge,
        });
    }

    // Options.
    let Some(Value::Array(raw)) = draft.get("options") else {
        return refuse("wrong number of options", "the item has no options");
    };
    if raw.len() < MIN_OPTIONS || raw.len() > MAX_OPTIONS {
        return refuse(
            "wrong number of options",
            format!("{} options: an item has {MIN_OPTIONS} to {MAX_OPTIONS}", raw.len()),
        );
    }
    let keys = raw.iter().filter(|o| o.get("correct").and_then(|c| c.as_bool()) == Some(true)).count();
    if keys != 1 {
        return refuse("not exactly one correct", format!("{keys} options are marked correct: exactly one is"));
    }
    let mut options = vec![];
    for (i, o) in raw.iter().enumerate() {
        let n = i + 1;
        let t = text(o.get("text"));
        let why = text(o.get("why"));
        let correct = o.get("correct").and_then(|c| c.as_bool()) == Some(true);
        if t.is_empty() {
            return refuse("empty option", format!("option {n} has no text"));
        }
        if t.chars().count() > MAX_OPTION {
            return refuse("too long", format!("option {n} is over {MAX_OPTION} characters"));
        }
        if why.is_empty() {
            return refuse("option without a why", format!("option {n} has no why"));
        }
        if POSITIONAL.is_match(&t) {
            return refuse(
                "position-dependent option",
                format!("option {n} (\"{t}\") depends on the others or their order"),
            );
        }
        let misconception = if correct { None } else { Some(text(o.get("misconception"))).filter(|m| !m.is_empty()) };
        if !correct && misconception.is_none() {
            return refuse(
                "wrong option without a misconception",
                format!("wrong option {n} doesn't say the misconception it stands for"),
            );
        }
        let foundation = if correct { None } else { resolve_foundation(o.get("foundation"), ctx)? };
        options.push(ItemOption { text: t, correct, why, misconception, foundation });
    }
    let mut seen = HashSet::new();
    for o in &options {
        if !seen.insert(norm(&o.text)) {
            return refuse("duplicate options", format!("two options say the same thing (\"{}\")", o.text));
        }
    }
    let key = options.iter().find(|o| o.correct).expect("exactly one key");
    let lengths: Vec<usize> = options.iter().map(|o| o.text.chars().count()).collect();
    let longest = *lengths.iter().max().unwrap_or(&0);
    let key_len = key.text.chars().count();
    if key_len == longest
        && lengths.iter().filter(|&&l| l == longest).count() == 1
        && key_len as f64 > LONGEST_CUE * median(&lengths)
    {
        return refuse("longest-option cue", "the correct option is the only long one: the length gives it away");
    }
    if key_len >= 12 && norm(&stem).contains(&norm(&key.text)) {
        return refuse("stem gives the answer", "the stem contains the correct option's text");
    }
    Ok(CheckedItem {
        kind,
        tier,
        concept_ids,
        stem,
        options: Some(options),
        exemplar: None,
        source_ref,
        model_knowledge,
    })
}

fn resolve_foundation(v: Option<&Value>, ctx: &CheckContext) -> Result<Option<Foundation>, Refusal> {
    let title = text(v);
    if title.is_empty() || ["none", "null", "n/a", "-"].contains(&title.to_lowercase().as_str()) {
        return Ok(None);
    }
    let on_map = ctx.topics.iter().find(|t| norm(t.title) == norm(&title));
    if let Some(t) = on_map
        && t.id == ctx.topic_id
    {
        return Err(Refusal {
            label: "foundation names its own topic",
            detail: format!("a foundation tag names this item's own topic (\"{}\")", t.title),
        });
    }
    Ok(Some(Foundation {
        topic_id: on_map.map(|t| t.id.to_string()),
        title: on_map.map(|t| t.title.to_string()).unwrap_or(title),
    }))
}

// ---- reading the model's replies -------------------------------------------------

pub const LETTERS: &str = "ABCDE";

/// The blind check's answer, mapped back through the order shown: the
/// option picked and any other it found defensible.
pub fn parse_blind(reply: &Value, order: &[usize]) -> Option<(usize, Vec<usize>)> {
    let letter = |v: &Value| -> Option<usize> {
        let c = v.as_str()?.trim().chars().next()?.to_ascii_uppercase();
        let i = LETTERS.find(c)?;
        order.get(i).copied()
    };
    let pick = letter(reply.get("pick")?)?;
    let also = reply
        .get("also_defensible")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(letter).filter(|&i| i != pick).collect())
        .unwrap_or_default();
    Some((pick, also))
}

/// Passes when the blind answer picks the key and finds nothing else
/// defensible; otherwise, why not.
pub fn blind_verdict(options: &[ItemOption], pick: usize, also: &[usize]) -> Option<String> {
    let key = options.iter().position(|o| o.correct)?;
    if pick != key {
        let picked = options.get(pick).map(|o| o.text.as_str()).unwrap_or("?");
        return Some(format!(
            "answered blind, the checker picked \"{picked}\", not the key: the key may be wrong or the item ambiguous"
        ));
    }
    if let Some(&other) = also.first() {
        let text = options.get(other).map(|o| o.text.as_str()).unwrap_or("?");
        return Some(format!("answered blind, the checker found another option defensible: \"{text}\""));
    }
    None
}

/// A random order for n options: the key's position carries nothing.
pub fn shuffled_order(n: usize, rng: &dyn crate::Rng) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = (rng.next_u128() % (i as u128 + 1)) as usize;
        order.swap(i, j);
    }
    order
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const CONCEPTS: [Concept; 3] = [
        Concept { id: "c1", name: "An image is immutable layers plus config" },
        Concept { id: "c2", name: "A tag is a movable name" },
        Concept { id: "c3", name: "A digest names exact content" },
    ];
    const TOPICS: [TopicRef; 2] =
        [TopicRef { id: "t1", title: "Container fundamentals" }, TopicRef { id: "t0", title: "Linux basics" }];

    fn ctx<'a>(kind: ItemKind, existing: &'a [(String, Vec<String>)]) -> CheckContext<'a> {
        CheckContext {
            kind,
            target: Tier::Evaluate,
            concepts: &CONCEPTS,
            slot_concept_id: (kind == ItemKind::Recall).then_some("c2"),
            existing,
            topics: &TOPICS,
            topic_id: "t1",
        }
    }

    fn good() -> Value {
        json!({
            "concepts": ["A tag is a movable name"],
            "tier": "understand",
            "stem": "A team re-pushes myapp:1.4 after a fix. What happens to a host that already has myapp:1.4?",
            "options": [
                {"text": "It keeps the old content until it pulls again", "correct": true, "why": "a pull resolves the tag again"},
                {"text": "It updates itself to the new content at once", "correct": false, "why": "nothing pushes to hosts", "misconception": "registries push updates", "foundation": "none"},
                {"text": "It refuses to run the old content any more", "correct": false, "why": "old layers stay valid", "misconception": "tags expire content", "foundation": "Linux basics"},
            ],
            "source": "OCI distribution spec"
        })
    }

    fn label(draft: &Value, kind: ItemKind) -> &'static str {
        check_item(draft, &ctx(kind, &[])).expect_err("refused").label
    }

    #[test]
    fn a_well_written_item_passes() {
        let item = check_item(&good(), &ctx(ItemKind::Recall, &[])).unwrap();
        assert_eq!(item.concept_ids, vec!["c2"]);
        assert_eq!(item.options.as_ref().unwrap()[2].foundation.as_ref().unwrap().topic_id.as_deref(), Some("t0"));
        assert!(!item.model_knowledge);
    }

    #[test]
    fn each_mechanical_flaw_is_refused_with_its_label() {
        let mut d = good();
        d["options"][2]["correct"] = json!(true);
        assert_eq!(label(&d, ItemKind::Recall), "not exactly one correct");

        let mut d = good();
        d["options"].as_array_mut().unwrap().truncate(2);
        assert_eq!(label(&d, ItemKind::Recall), "wrong number of options");

        let mut d = good();
        d["options"][1]["why"] = json!("");
        assert_eq!(label(&d, ItemKind::Recall), "option without a why");

        let mut d = good();
        d["options"][1]["misconception"] = json!("");
        assert_eq!(label(&d, ItemKind::Recall), "wrong option without a misconception");

        let mut d = good();
        d["options"][1]["text"] = json!("All of the above");
        assert_eq!(label(&d, ItemKind::Recall), "position-dependent option");

        let mut d = good();
        d["options"][0]["text"] = json!(
            "It keeps the old content cached locally until something explicitly pulls the tag again from the registry"
        );
        assert_eq!(label(&d, ItemKind::Recall), "longest-option cue");

        let mut d = good();
        d["stem"] =
            json!("It keeps the old content until it pulls again, right? What happens to a host with myapp:1.4?");
        assert_eq!(label(&d, ItemKind::Recall), "stem gives the answer");

        let mut d = good();
        d["concepts"] = json!(["Something else entirely"]);
        assert_eq!(label(&d, ItemKind::Recall), "concept not on the outcome");

        let mut d = good();
        d["tier"] = json!("evaluate");
        assert_eq!(label(&d, ItemKind::Recall), "tier outside the kind");

        let mut d = good();
        d["options"][2]["foundation"] = json!("Container fundamentals");
        assert_eq!(label(&d, ItemKind::Recall), "foundation names its own topic");
    }

    #[test]
    fn scenarios_need_two_concepts_at_the_right_tier() {
        let mut d = good();
        d["tier"] = json!("apply");
        assert_eq!(label(&d, ItemKind::Apply), "too few concepts");
        d["concepts"] = json!(["c1", "c2"]);
        assert!(check_item(&d, &ctx(ItemKind::Apply, &[])).is_ok());
        assert_eq!(label(&d, ItemKind::Target), "tier outside the kind");
    }

    #[test]
    fn a_near_duplicate_stem_is_refused() {
        let existing = vec![(good()["stem"].as_str().unwrap().to_string(), vec!["c2".to_string()])];
        assert_eq!(check_item(&good(), &ctx(ItemKind::Recall, &existing)).unwrap_err().label, "near-duplicate stem");
    }

    #[test]
    fn explain_items_need_an_exemplar() {
        let d = json!({"stem": "Explain to a customer why their service changed overnight with the same tag."});
        assert_eq!(label(&d, ItemKind::Explain), "no exemplar");
        let d = json!({"stem": "Explain it.", "exemplar": "Because tags move."});
        let item = check_item(&d, &ctx(ItemKind::Explain, &[])).unwrap();
        assert_eq!(item.concept_ids.len(), 3);
        assert!(item.model_knowledge);
    }

    #[test]
    fn the_blind_check_needs_the_key_and_nothing_else() {
        let opts = check_item(&good(), &ctx(ItemKind::Recall, &[])).unwrap().options.unwrap();
        let order = vec![2, 0, 1];
        let (pick, also) = parse_blind(&json!({"pick": "B", "also_defensible": []}), &order).unwrap();
        assert_eq!(pick, 0);
        assert!(blind_verdict(&opts, pick, &also).is_none());
        let (pick, also) = parse_blind(&json!({"pick": "a"}), &order).unwrap();
        assert!(blind_verdict(&opts, pick, &also).unwrap().contains("not the key"));
        let (pick, also) = parse_blind(&json!({"pick": "B", "also_defensible": ["C"]}), &order).unwrap();
        assert!(blind_verdict(&opts, pick, &also).unwrap().contains("defensible"));
    }

    #[test]
    fn bank_needs_top_up_low_slots() {
        let concepts = vec!["c1".to_string(), "c2".to_string()];
        let needs = bank_needs(Tier::Evaluate, &concepts, &[], &HashSet::new());
        assert_eq!(needs.len(), 5); // 2 recall, apply, target, explain
        assert_eq!(bank_size(Tier::Evaluate, 2), 8 + 4 + 4 + 2);
        assert_eq!(bank_size(Tier::Apply, 2), 8 + 4 + 2);
    }

    #[test]
    fn a_partly_filled_slot_is_written_to_full_size() {
        let concepts = vec!["c1".to_string()];
        let ids: Vec<String> = (0..3).map(|i| format!("i{i}")).collect();
        let concept = vec!["c1".to_string()];
        let three: Vec<BankedItem> =
            ids.iter().map(|id| BankedItem { id, kind: ItemKind::Recall, concept_ids: &concept }).collect();
        let needs = bank_needs(Tier::Understand, &concepts, &three, &HashSet::new());
        let recall = needs.iter().find(|(s, _)| s.kind == ItemKind::Recall).expect("recall still needs one");
        assert_eq!(recall.1, 1, "3 of 4 written: one more, not a refill decision");
        // Practised down to one unseen: topped back up.
        let attempted: HashSet<String> = ids[..2].iter().cloned().collect();
        let mut four = three;
        four.push(BankedItem { id: "i3", kind: ItemKind::Recall, concept_ids: &concept });
        let needs = bank_needs(Tier::Understand, &concepts, &four, &attempted);
        assert!(needs.iter().all(|(s, _)| s.kind != ItemKind::Recall), "two unseen of four: no refill yet");
    }

    #[test]
    fn a_new_topic_gets_its_first_set_before_an_older_one_fills_up() {
        let none: &'static HashSet<String> = Box::leak(Box::new(HashSet::new()));
        let old: &'static [String] = Box::leak(vec!["o1".to_string()].into_boxed_slice());
        let new: &'static [String] = Box::leak(vec!["n1".to_string()].into_boxed_slice());
        let rec = |c: &'static [String], i: usize| BankedItem {
            id: Box::leak(format!("{}-{i}", c[0]).into_boxed_str()),
            kind: ItemKind::Recall,
            concept_ids: &c[..1],
        };
        let bank = |concepts: &'static [String], active: Vec<BankedItem<'static>>| {
            vec![OutcomeBank { target: Tier::Evaluate, concept_ids: concepts, active, attempted: none }]
        };
        // The older topic has its first set and second recall; the new one
        // has nothing: the new one's first set comes first.
        let older = bank(old, vec![rec(old, 0), rec(old, 1)]);
        let (t, _, slot, _) = next_across(&[older, bank(new, vec![])]).unwrap();
        assert_eq!((t, slot.concept_id.as_deref()), (1, Some("n1")));
        // Both need the same thing: the older topic first.
        let (t, ..) = next_across(&[bank(old, vec![]), bank(new, vec![])]).unwrap();
        assert_eq!(t, 0);
        // Nothing needed anywhere: nothing to write.
        assert!(next_across(&[vec![], vec![]]).is_none());
    }

    #[test]
    fn questions_are_written_in_the_order_theyll_be_needed() {
        let a = vec!["a1".to_string(), "a2".to_string()];
        let b = vec!["b1".to_string()];
        let none = HashSet::new();
        let bank = |concepts: &'static [String], active: Vec<BankedItem<'static>>| OutcomeBank {
            target: Tier::Evaluate,
            concept_ids: concepts,
            active,
            attempted: Box::leak(Box::new(none.clone())),
        };
        let a: &'static [String] = Box::leak(a.into_boxed_slice());
        let b: &'static [String] = Box::leak(b.into_boxed_slice());
        let rec = |c: &'static [String], i: usize| BankedItem {
            id: Box::leak(format!("{}-{i}", c[0]).into_boxed_str()),
            kind: ItemKind::Recall,
            concept_ids: &c[..1],
        };
        // Nothing yet: outcome 1's first concept, one question.
        let (i, slot, n) = next_need(&[bank(a, vec![]), bank(b, vec![])]).unwrap();
        assert_eq!((i, slot.concept_id.as_deref(), n), (0, Some("a1"), 1));
        // Outcome 1 has placement on a1 only: a2 next.
        let (i, slot, _) = next_need(&[bank(a, vec![rec(a, 0)]), bank(b, vec![])]).unwrap();
        assert_eq!((i, slot.concept_id.as_deref()), (0, Some("a2")));
        assert!(!placement_ready(a, &[rec(a, 0)]));
        // Outcome 1 placement done: second recall on outcome 1 before outcome 2's placement.
        let a2 = BankedItem { id: "a2-0", kind: ItemKind::Recall, concept_ids: &a[1..2] };
        let (i, slot, _) = next_need(&[bank(a, vec![rec(a, 0), a2]), bank(b, vec![])]).unwrap();
        assert_eq!((i, slot.concept_id.as_deref()), (0, Some("a1")));
        assert!(placement_ready(
            a,
            &[rec(a, 0), BankedItem { id: "x", kind: ItemKind::Recall, concept_ids: &a[1..2] }]
        ));
    }

    #[test]
    fn shuffles_are_reproducible_and_complete() {
        let a = shuffled_order(4, &crate::SeededRng::from_seed(3));
        assert_eq!(a, shuffled_order(4, &crate::SeededRng::from_seed(3)));
        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3]);
    }
}
