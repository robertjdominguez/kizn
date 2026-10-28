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

/// One outcome's bank, as far as deciding what to write next goes.
pub struct OutcomeBank<'a> {
    pub target: Tier,
    pub concept_ids: &'a [String],
    pub active: Vec<BankedItem<'a>>,
    pub attempted: &'a HashSet<String>,
    /// Concepts the learner has answered a question on (any outcome).
    pub practised: &'a HashSet<String>,
}

/// Unseen recall questions kept ready for each concept in reach: one for
/// the next time it comes up, and a spare (one may be refused or disputed,
/// and a recall must never repeat the last question).
pub const AHEAD: usize = 2;

/// What practice can reach in the next few days, and what's still to write
/// for it, written just in time rather than to a full bank up front (the
/// learner meets at most MAX_NEW new concepts and DAILY_RECALL_CAP recalls a
/// day). In reach: every concept practised, and every concept on the
/// outcome being worked (the first with one not yet practised) and on the
/// next. Each gets AHEAD unseen recall questions. Nothing is written for
/// outcomes further on, or of kinds practice doesn't serve yet (use it,
/// target, explain): those get their own reach when the deck does.
///
/// Returned as (rank, outcome, slot, count), ranked by how soon practice
/// needs it: 0 the worked outcome's first set; 1 practised concepts with no
/// unseen question; 2 the worked outcome's second question; 3 practised
/// concepts topped up; 4 and 5 the next outcome's first and second.
pub fn needs_ahead(outcomes: &[OutcomeBank]) -> Vec<(u8, usize, Slot, usize)> {
    let worked = outcomes.iter().position(|o| o.concept_ids.iter().any(|c| !o.practised.contains(c)));
    let mut out = vec![];
    for (i, o) in outcomes.iter().enumerate() {
        let ahead = worked.is_some_and(|w| i == w || i == w + 1);
        let first = worked == Some(i);
        for c in o.concept_ids {
            let in_slot: Vec<&BankedItem> =
                o.active.iter().filter(|it| it.kind == ItemKind::Recall && it.concept_ids.first() == Some(c)).collect();
            let fresh = in_slot.iter().filter(|it| !o.attempted.contains(it.id)).count();
            let slot = Slot { kind: ItemKind::Recall, concept_id: Some(c.clone()) };
            if o.practised.contains(c) {
                match fresh {
                    0 => out.push((1, i, slot, AHEAD)),
                    n if n < AHEAD => out.push((3, i, slot, AHEAD - n)),
                    _ => {}
                }
            } else if ahead {
                let (first_rank, second_rank) = if first { (0, 2) } else { (4, 5) };
                let have = in_slot.len();
                // The first question (for the first set) before the rest.
                if have == 0 {
                    out.push((first_rank, i, slot.clone(), 1));
                }
                if have.max(1) < AHEAD {
                    out.push((second_rank, i, slot, AHEAD - have.max(1)));
                }
            }
        }
    }
    out
}

/// The questions an outcome is meant to hold right now: what it has, plus
/// what's still to write for what practice can reach (for "N written").
pub fn ahead_size(outcomes: &[OutcomeBank], i: usize) -> usize {
    let written = outcomes[i].active.len();
    written + needs_ahead(outcomes).iter().filter(|(_, o, ..)| *o == i).map(|(.., n)| n).sum::<usize>()
}

/// What to write next across a topic's outcomes: the soonest of
/// `needs_ahead`. Returns the outcome's index, the slot and how many.
pub fn next_need(outcomes: &[OutcomeBank]) -> Option<(usize, Slot, usize)> {
    ranked_need(outcomes).map(|(_, i, slot, n)| (i, slot, n))
}

/// Like `next_need`, with how soon the learner needs it (`needs_ahead`'s
/// rank).
pub fn ranked_need(outcomes: &[OutcomeBank]) -> Option<(u8, usize, Slot, usize)> {
    needs_ahead(outcomes).into_iter().min_by_key(|(rank, i, ..)| (*rank, *i))
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

    /// Banks for tests: each outcome's concepts and items, nobody's
    /// answered anything unless `practised` says so.
    fn concepts(names: &[&str]) -> &'static [String] {
        Box::leak(names.iter().map(|n| n.to_string()).collect::<Vec<_>>().into_boxed_slice())
    }

    fn rec(c: &'static [String], at: usize, i: usize) -> BankedItem<'static> {
        BankedItem {
            id: Box::leak(format!("{}-{i}", c[at]).into_boxed_str()),
            kind: ItemKind::Recall,
            concept_ids: &c[at..at + 1],
        }
    }

    fn bank(
        concepts: &'static [String],
        active: Vec<BankedItem<'static>>,
        attempted: &'static HashSet<String>,
        practised: &'static HashSet<String>,
    ) -> OutcomeBank<'static> {
        OutcomeBank { target: Tier::Evaluate, concept_ids: concepts, active, attempted, practised }
    }

    fn set(ids: &[&str]) -> &'static HashSet<String> {
        Box::leak(Box::new(ids.iter().map(|s| s.to_string()).collect()))
    }

    #[test]
    fn questions_are_written_in_the_order_theyll_be_needed() {
        let (a, b) = (concepts(&["a1", "a2"]), concepts(&["b1"]));
        let none = set(&[]);
        // Nothing yet: outcome 1's first concept, one question.
        let (i, slot, n) = next_need(&[bank(a, vec![], none, none), bank(b, vec![], none, none)]).unwrap();
        assert_eq!((i, slot.concept_id.as_deref(), n), (0, Some("a1"), 1));
        // Outcome 1 has its first question on a1 only: a2 next.
        let (i, slot, _) = next_need(&[bank(a, vec![rec(a, 0, 0)], none, none), bank(b, vec![], none, none)]).unwrap();
        assert_eq!((i, slot.concept_id.as_deref()), (0, Some("a2")));
        assert!(!placement_ready(a, &[rec(a, 0, 0)]));
        // Outcome 1's first set done: its second questions before outcome 2's first.
        let first_set = || vec![rec(a, 0, 0), rec(a, 1, 0)];
        let (i, slot, _) = next_need(&[bank(a, first_set(), none, none), bank(b, vec![], none, none)]).unwrap();
        assert_eq!((i, slot.concept_id.as_deref()), (0, Some("a1")));
        assert!(placement_ready(a, &first_set()));
    }

    #[test]
    fn only_what_practice_can_reach_is_written() {
        let (a, b, c) = (concepts(&["a1"]), concepts(&["b1"]), concepts(&["c1"]));
        let none = set(&[]);
        let needs =
            needs_ahead(&[bank(a, vec![], none, none), bank(b, vec![], none, none), bank(c, vec![], none, none)]);
        // The worked outcome and the next; nothing for the third, and only recall.
        assert!(needs.iter().all(|(_, o, ..)| *o < 2), "{needs:?}");
        assert!(needs.iter().all(|(_, _, slot, _)| slot.kind == ItemKind::Recall));
        // Two questions per concept in reach, then nothing more.
        let full = || vec![rec(a, 0, 0), rec(a, 0, 1)];
        let fullb = || vec![rec(b, 0, 0), rec(b, 0, 1)];
        assert!(needs_ahead(&[bank(a, full(), none, none), bank(b, fullb(), none, none)]).is_empty());
        assert_eq!(ahead_size(&[bank(a, vec![], none, none), bank(b, vec![], none, none)], 0), AHEAD);
    }

    #[test]
    fn practised_concepts_are_topped_up_and_the_next_outcome_comes_in_reach() {
        let (a, b, c) = (concepts(&["a1", "a2"]), concepts(&["b1"]), concepts(&["c1"]));
        // a1 answered on both its questions; a2 not yet practised.
        let attempted = set(&["a1-0", "a1-1"]);
        let practised = set(&["a1"]);
        let outcomes = [
            bank(a, vec![rec(a, 0, 0), rec(a, 0, 1), rec(a, 1, 0)], attempted, practised),
            bank(b, vec![], attempted, practised),
            bank(c, vec![], attempted, practised),
        ];
        // a1 has nothing unseen left: before a2's second question.
        let (rank, i, slot, n) = ranked_need(&outcomes).unwrap();
        assert_eq!((rank, i, slot.concept_id.as_deref(), n), (1, 0, Some("a1"), AHEAD));
        // Outcome 1 all practised: outcome 2 is worked, outcome 3 comes in reach.
        let practised = set(&["a1", "a2"]);
        let outcomes = [
            bank(a, vec![], attempted, practised),
            bank(b, vec![], attempted, practised),
            bank(c, vec![], attempted, practised),
        ];
        let needs = needs_ahead(&outcomes);
        assert!(needs.iter().any(|(r, o, ..)| *r == 0 && *o == 1), "outcome 2's first set: {needs:?}");
        assert!(needs.iter().any(|(_, o, ..)| *o == 2), "outcome 3 in reach: {needs:?}");
    }

    #[test]
    fn a_new_topic_gets_its_first_set_before_an_older_one_tops_up() {
        let (old, new) = (concepts(&["o1"]), concepts(&["n1"]));
        let none = set(&[]);
        // The older topic has two questions ready on its concept, one seen;
        // the new one has nothing: the new one's first set comes first.
        let (attempted, practised) = (set(&["o1-0"]), set(&["o1"]));
        let older = vec![bank(old, vec![rec(old, 0, 0), rec(old, 0, 1)], attempted, practised)];
        let (t, _, slot, _) = next_across(&[older, vec![bank(new, vec![], none, none)]]).unwrap();
        assert_eq!((t, slot.concept_id.as_deref()), (1, Some("n1")));
        // Both need the same thing: the older topic first.
        let (t, ..) = next_across(&[vec![bank(old, vec![], none, none)], vec![bank(new, vec![], none, none)]]).unwrap();
        assert_eq!(t, 0);
        // Nothing needed anywhere: nothing to write.
        assert!(next_across(&[vec![], vec![]]).is_none());
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
