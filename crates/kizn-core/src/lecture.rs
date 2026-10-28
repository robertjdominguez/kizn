//! A lecture: one concept explained in depth, for reading, where every part
//! is backed by a source kizn read. The model writes it; code checks that
//! every paragraph, key term and mix-up cites sources kizn holds, that it
//! doesn't give away the bank's questions, and (with the checker's verdicts)
//! leaves out what its sources don't support.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// What a lecture is written as, and stored as (cites are source ids).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lecture {
    /// A short title for the reading (the concept's statement can be long).
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub key_terms: Vec<Term>,
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub misconceptions: Vec<MixUp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub term: String,
    pub meaning: String,
    #[serde(default)]
    pub cites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    #[serde(default)]
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paragraph {
    pub text: String,
    #[serde(default)]
    pub cites: Vec<String>,
}

/// A common mistaken belief about the concept, and what's true instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixUp {
    pub belief: String,
    pub correction: String,
    #[serde(default)]
    pub cites: Vec<String>,
}

/// One part of a lecture the checker verifies against what it cites: a
/// paragraph ("p<section>.<paragraph>"), a key term ("t<n>") or a mix-up
/// ("m<n>").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub cites: Vec<String>,
}

/// Fewest sources a lecture is written from: one source can be wrong or
/// partial; two that agree are a basis to read from.
pub const MIN_SOURCES: usize = 2;
/// Fewest paragraphs a lecture keeps, once unsupported ones are left out.
pub const MIN_PARAGRAPHS: usize = 2;
/// Words in a row shared with a question's stem or right answer that count
/// as giving it away.
const GIVEAWAY_WORDS: usize = 8;

impl Lecture {
    /// Every part to verify, with what it cites.
    pub fn claims(&self) -> Vec<Claim> {
        let mut out = vec![];
        for (s, section) in self.sections.iter().enumerate() {
            for (p, para) in section.paragraphs.iter().enumerate() {
                out.push(Claim { id: format!("p{s}.{p}"), text: para.text.clone(), cites: para.cites.clone() });
            }
        }
        for (i, t) in self.key_terms.iter().enumerate() {
            out.push(Claim { id: format!("t{i}"), text: format!("{}: {}", t.term, t.meaning), cites: t.cites.clone() });
        }
        for (i, m) in self.misconceptions.iter().enumerate() {
            out.push(Claim {
                id: format!("m{i}"),
                text: format!("Mistaken: {} Correct: {}", m.belief, m.correction),
                cites: m.cites.clone(),
            });
        }
        out
    }

    /// The lecture without the claims named (unsupported by their sources);
    /// a section left with no paragraphs goes too.
    pub fn without(&self, unsupported: &HashSet<String>) -> Lecture {
        let sections = self
            .sections
            .iter()
            .enumerate()
            .map(|(s, section)| Section {
                heading: section.heading.clone(),
                paragraphs: section
                    .paragraphs
                    .iter()
                    .enumerate()
                    .filter(|(p, _)| !unsupported.contains(&format!("p{s}.{p}")))
                    .map(|(_, para)| para.clone())
                    .collect(),
            })
            .filter(|s| !s.paragraphs.is_empty())
            .collect();
        let keep = |prefix: &str, i: usize| !unsupported.contains(&format!("{prefix}{i}"));
        Lecture {
            title: self.title.clone(),
            key_terms: self
                .key_terms
                .iter()
                .enumerate()
                .filter(|(i, _)| keep("t", *i))
                .map(|(_, t)| t.clone())
                .collect(),
            sections,
            misconceptions: self
                .misconceptions
                .iter()
                .enumerate()
                .filter(|(i, _)| keep("m", *i))
                .map(|(_, m)| m.clone())
                .collect(),
        }
    }

    /// The same lecture with each claim citing what backs it (`cites`, by
    /// claim id); a claim not listed cites nothing.
    pub fn with_cites(&self, cites: &std::collections::HashMap<String, Vec<String>>) -> Lecture {
        let c = |id: String| cites.get(&id).cloned().unwrap_or_default();
        Lecture {
            title: self.title.clone(),
            key_terms: self
                .key_terms
                .iter()
                .enumerate()
                .map(|(i, t)| Term { term: t.term.clone(), meaning: t.meaning.clone(), cites: c(format!("t{i}")) })
                .collect(),
            sections: self
                .sections
                .iter()
                .enumerate()
                .map(|(si, sec)| Section {
                    heading: sec.heading.clone(),
                    paragraphs: sec
                        .paragraphs
                        .iter()
                        .enumerate()
                        .map(|(pi, p)| Paragraph { text: p.text.clone(), cites: c(format!("p{si}.{pi}")) })
                        .collect(),
                })
                .collect(),
            misconceptions: self
                .misconceptions
                .iter()
                .enumerate()
                .map(|(i, m)| MixUp {
                    belief: m.belief.clone(),
                    correction: m.correction.clone(),
                    cites: c(format!("m{i}")),
                })
                .collect(),
        }
    }

    /// The sources cited, in the order they're first cited: footnote n is
    /// the nth of these.
    pub fn footnotes(&self) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for c in self.claims() {
            for id in c.cites {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    /// The same lecture with every cite mapped (the model's short ids to
    /// kizn's source ids, say); cites with no mapping are dropped.
    pub fn map_cites(&self, map: impl Fn(&str) -> Option<String>) -> Lecture {
        let m = |cites: &[String]| cites.iter().filter_map(|c| map(c)).collect::<Vec<_>>();
        Lecture {
            title: self.title.clone(),
            key_terms: self
                .key_terms
                .iter()
                .map(|t| Term { term: t.term.clone(), meaning: t.meaning.clone(), cites: m(&t.cites) })
                .collect(),
            sections: self
                .sections
                .iter()
                .map(|s| Section {
                    heading: s.heading.clone(),
                    paragraphs: s
                        .paragraphs
                        .iter()
                        .map(|p| Paragraph { text: p.text.clone(), cites: m(&p.cites) })
                        .collect(),
                })
                .collect(),
            misconceptions: self
                .misconceptions
                .iter()
                .map(|x| MixUp { belief: x.belief.clone(), correction: x.correction.clone(), cites: m(&x.cites) })
                .collect(),
        }
    }
}

/// Why a lecture can't be shown as written, or nothing if it can: every
/// part cites at least one source kizn holds (`held`), it draws on at least
/// MIN_SOURCES of them, it has MIN_PARAGRAPHS paragraphs and a key term,
/// and it doesn't repeat GIVEAWAY_WORDS words in a row from a question's
/// stem or right answer (`answers`), so reading doesn't turn practice into
/// recognising the lecture's wording. The concept's own statement doesn't
/// count: lecture and questions both name it.
pub fn check_lecture(l: &Lecture, held: &[String], answers: &[String], concept: &str) -> Result<(), String> {
    let paragraphs: usize = l.sections.iter().map(|s| s.paragraphs.len()).sum();
    if paragraphs < MIN_PARAGRAPHS {
        return Err(format!("too short: {paragraphs} paragraphs, at least {MIN_PARAGRAPHS}"));
    }
    if l.key_terms.is_empty() {
        return Err("no key terms".into());
    }
    for c in l.claims() {
        if c.text.trim().is_empty() {
            return Err(format!("{} is empty", c.id));
        }
        if c.cites.is_empty() {
            return Err(format!("{} cites nothing", c.id));
        }
        if let Some(tag) = source_tag(&c.text) {
            return Err(format!("{} names a source in its text ({tag}): footnotes do that", c.id));
        }
        if let Some(bad) = c.cites.iter().find(|id| !held.contains(id)) {
            return Err(format!("{} cites {bad:?}, which isn't one of the sources", c.id));
        }
        if let Some(a) = answers.iter().find(|a| shares_run(&c.text, a, GIVEAWAY_WORDS, concept)) {
            return Err(format!("{} repeats a question's wording: {a:?}", c.id));
        }
    }
    let cited = l.footnotes().len();
    if cited < MIN_SOURCES {
        return Err(format!("draws on {cited} source, at least {MIN_SOURCES}"));
    }
    Ok(())
}

/// A source id written into the prose ("(s2)", "[s1]"), if there is one.
fn source_tag(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    for (i, &open) in chars.iter().enumerate() {
        let close = match open {
            '(' => ')',
            '[' => ']',
            _ => continue,
        };
        if chars.get(i + 1).is_some_and(|c| *c == 's' || *c == 'S') {
            let digits: String = chars[i + 2..].iter().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() && chars.get(i + 2 + digits.len()) == Some(&close) {
                return Some(format!("{open}s{digits}{close}"));
            }
        }
    }
    None
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// The words, with every occurrence of `phrase` broken out (so no run
/// crosses or includes it).
fn words_without(s: &str, phrase: &[String]) -> Vec<String> {
    let w = words(s);
    if phrase.is_empty() {
        return w;
    }
    let mut out = vec![];
    let mut i = 0;
    while i < w.len() {
        if w[i..].starts_with(phrase) {
            out.push(format!("\u{0}{i}"));
            i += phrase.len();
        } else {
            out.push(w[i].clone());
            i += 1;
        }
    }
    out
}

/// Whether `a` and `b` share `n` words in a row, not counting `except`.
fn shares_run(a: &str, b: &str, n: usize, except: &str) -> bool {
    let phrase = words(except);
    let (a, b) = (words_without(a, &phrase), words_without(b, &phrase));
    if a.len() < n || b.len() < n {
        return false;
    }
    let runs: HashSet<&[String]> = b.windows(n).collect();
    a.windows(n).any(|w| runs.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(text: &str, cites: &[&str]) -> Paragraph {
        Paragraph { text: text.into(), cites: cites.iter().map(|s| s.to_string()).collect() }
    }

    fn lecture() -> Lecture {
        Lecture {
            title: "Images, layer by layer".into(),
            key_terms: vec![Term {
                term: "Layer".into(),
                meaning: "a read-only filesystem diff".into(),
                cites: vec!["a".into()],
            }],
            sections: vec![
                Section {
                    heading: "What an image is".into(),
                    paragraphs: vec![
                        para("An image is a stack of read-only layers.", &["a"]),
                        para("Each layer records what changed.", &["b"]),
                    ],
                },
                Section {
                    heading: "Why it matters".into(),
                    paragraphs: vec![para("Layers are shared between images.", &["a", "b"])],
                },
            ],
            misconceptions: vec![MixUp {
                belief: "An image is a running process.".into(),
                correction: "A container is the running process; the image is its template.".into(),
                cites: vec!["b".into()],
            }],
        }
    }

    fn held() -> Vec<String> {
        vec!["a".into(), "b".into()]
    }

    #[test]
    fn a_lecture_backed_by_its_sources_passes() {
        assert_eq!(check_lecture(&lecture(), &held(), &[], ""), Ok(()));
        assert_eq!(lecture().footnotes(), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn every_part_must_cite_a_source_kizn_holds() {
        let mut l = lecture();
        l.sections[0].paragraphs[1].cites.clear();
        assert!(check_lecture(&l, &held(), &[], "").unwrap_err().contains("p0.1 cites nothing"));
        let mut l = lecture();
        l.misconceptions[0].cites = vec!["made-up".into()];
        assert!(check_lecture(&l, &held(), &[], "").unwrap_err().contains("isn't one of the sources"));
    }

    #[test]
    fn one_source_isnt_enough() {
        let l = lecture().map_cites(|_| Some("a".into()));
        assert!(check_lecture(&l, &held(), &[], "").unwrap_err().contains("draws on 1 source"));
    }

    #[test]
    fn a_lecture_mustnt_give_a_question_away() {
        let answers =
            vec!["Which is true? Every image is built from stacked, read-only layers a container adds to".to_string()];
        let mut l = lecture();
        l.sections[0].paragraphs[0].text =
            "Every image is built from stacked, read-only layers a container adds to.".into();
        assert!(check_lecture(&l, &held(), &answers, "").unwrap_err().contains("repeats a question's wording"));
        // The concept's own statement doesn't count.
        let named = vec!["Which statement about an image as a stack of read-only layers holds".to_string()];
        let mut l = lecture();
        l.sections[0].paragraphs[0].text = "Think of an image as a stack of read-only layers.".into();
        assert_eq!(check_lecture(&l, &held(), &named, "an image as a stack of read-only layers"), Ok(()));
        // A few shared words are fine.
        assert_eq!(check_lecture(&lecture(), &held(), &answers, ""), Ok(()));
    }

    #[test]
    fn sources_are_footnotes_not_prose() {
        let mut l = lecture();
        l.sections[0].paragraphs[0].text = "GCC sets the width (s2).".into();
        assert!(check_lecture(&l, &held(), &[], "").unwrap_err().contains("names a source in its text"));
        // An S3 bucket is just a word.
        l.sections[0].paragraphs[0].text = "Logs land in an S3 bucket (S3 is object storage).".into();
        assert_eq!(check_lecture(&l, &held(), &[], ""), Ok(()));
    }

    #[test]
    fn unsupported_parts_are_left_out() {
        let unsupported: HashSet<String> = ["p1.0".to_string(), "m0".to_string()].into();
        let l = lecture().without(&unsupported);
        assert_eq!(l.sections.len(), 1, "a section with nothing left goes");
        assert!(l.misconceptions.is_empty());
        assert_eq!(l.claims().len(), 3);
    }
}
