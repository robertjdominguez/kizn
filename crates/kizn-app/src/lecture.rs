//! Lectures: one concept explained in depth, written just in time (when the
//! concept is opened or missed), every part citing a source kizn read and
//! the checker verified. The model writes and judges; code checks the cites,
//! that no question is given away, and leaves out what isn't supported.

use std::collections::{HashMap, HashSet};

use kizn_core::bank::ItemOption;
use kizn_core::lecture::{Lecture, MIN_SOURCES, check_lecture};
use serde_json::{Value, json};

use crate::command::{FootnoteView, LectureView, MixUpView, ParagraphView, Role, SectionView, TermView};
use crate::{AppError, Ctx};

const WRITE_LECTURE: &str = "kizn-task: write-lecture
You write a lecture for one adult learner on ONE concept, to read when they need it (often after missing a question on it). Teach it in depth: what it is, how it works, why it matters for what they're learning, a concrete example, and where people go wrong.

Rules:
- Write from the excerpts of the sources given. You may use your own knowledge to explain and connect, but every paragraph, key term and mix-up must be supported by the excerpts it cites. Cite sources by their id (\"s1\").
- Cite at least two different sources across the lecture.
- 3 to 6 key terms, each with a plain meaning.
- 2 to 4 sections, each with 1 to 3 short paragraphs.
- In \"misconceptions\", take on the mistaken beliefs listed: the belief, then what's true, with cites.
- Don't write practice questions and don't echo answer options: this is for understanding, not for recognising answers.
- If \"supports\" lists topics, explain the concept in its own terms; one of them may appear as one example among others.

Reply with JSON only: {\"key_terms\": [{\"term\": \"...\", \"meaning\": \"...\", \"cites\": [\"s1\"]}], \"sections\": [{\"heading\": \"...\", \"paragraphs\": [{\"text\": \"...\", \"cites\": [\"s1\", \"s2\"]}]}], \"misconceptions\": [{\"belief\": \"...\", \"correction\": \"...\", \"cites\": [\"s2\"]}]}";

const VERIFY_LECTURE: &str = "kizn-task: verify-lecture
For each claim, say whether the excerpts it cites support it: every fact in it is stated in, or directly follows from, those excerpts. Rephrasing or explaining supported facts is fine; a fact the excerpts don't contain is not supported, however well known.

Reply with JSON only: {\"verdicts\": [{\"id\": \"p0.0\", \"supported\": true, \"why\": \"...\"}]}";

fn cites() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

fn lecture_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "key_terms": {"type": "array", "items": {
                "type": "object",
                "properties": {"term": {"type": "string"}, "meaning": {"type": "string"}, "cites": cites()},
                "required": ["term", "meaning", "cites"], "additionalProperties": false
            }},
            "sections": {"type": "array", "items": {
                "type": "object",
                "properties": {"heading": {"type": "string"}, "paragraphs": {"type": "array", "items": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}, "cites": cites()},
                    "required": ["text", "cites"], "additionalProperties": false
                }}},
                "required": ["heading", "paragraphs"], "additionalProperties": false
            }},
            "misconceptions": {"type": "array", "items": {
                "type": "object",
                "properties": {"belief": {"type": "string"}, "correction": {"type": "string"}, "cites": cites()},
                "required": ["belief", "correction", "cites"], "additionalProperties": false
            }}
        },
        "required": ["key_terms", "sections", "misconceptions"], "additionalProperties": false
    })
}

fn verdicts_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"verdicts": {"type": "array", "items": {
            "type": "object",
            "properties": {"id": {"type": "string"}, "supported": {"type": "boolean"}, "why": {"type": "string"}},
            "required": ["id", "supported", "why"], "additionalProperties": false
        }}},
        "required": ["verdicts"], "additionalProperties": false
    })
}

/// A source as the author sees it: a short id, its title and excerpts.
struct Held {
    short: String,
    source_id: String,
    title: String,
    excerpts: Vec<String>,
}

impl Ctx {
    /// Writes a concept's lecture in the background, once at a time.
    pub(crate) fn start_lecture(&self, concept_id: &str) {
        if !self.lecturing.lock().unwrap().insert(concept_id.to_string()) {
            return;
        }
        let ctx = self.clone();
        let id = concept_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = ctx.write_lecture(&id).await {
                let _ = ctx.save_lecture(&id, "failed", None, Some(&e.to_string()));
            }
            ctx.lecturing.lock().unwrap().remove(&id);
            if let Ok(Some(c)) = ctx.store.concept(&id)
                && let Ok(Some(o)) = ctx.store.outcome(&c.outcome_id)
            {
                ctx.changed(&o.topic_id);
            }
        });
    }

    fn save_lecture(
        &self,
        concept_id: &str,
        status: &str,
        body: Option<&Lecture>,
        reason: Option<&str>,
    ) -> Result<(), AppError> {
        self.store
            .save_lecture(&kizn_store::LectureRow {
                concept_id: concept_id.into(),
                status: status.into(),
                body_json: body.and_then(|b| serde_json::to_string(b).ok()),
                reason: reason.map(str::to_string),
                written_at: self.now(),
            })
            .map_err(AppError::from)
    }

    /// The sources a lecture can cite: verified, with text kizn holds, and
    /// the excerpts that best match the concept.
    fn held_sources(&self, concept_id: &str, statement: &str) -> Result<Vec<Held>, AppError> {
        let mut out = vec![];
        for s in self.store.sources(concept_id)? {
            let Some(content) = s.content_json.as_deref().filter(|_| s.status == "verified") else {
                continue;
            };
            let chunks = if s.kind == "video" {
                let cues: Vec<kizn_core::source::Cue> = serde_json::from_str(content).unwrap_or_default();
                kizn_core::source::cue_chunks(&cues, 60.0)
            } else {
                let blocks: Vec<kizn_core::source::Block> = serde_json::from_str(content).unwrap_or_default();
                kizn_core::source::page_chunks(&blocks, 120)
            };
            let excerpts: Vec<String> =
                kizn_core::source::relevant(&chunks, statement, 4).into_iter().map(|c| c.text).collect();
            if excerpts.is_empty() {
                continue;
            }
            out.push(Held { short: format!("s{}", out.len() + 1), source_id: s.id, title: s.title, excerpts });
        }
        Ok(out)
    }

    async fn write_lecture(&self, concept_id: &str) -> Result<(), AppError> {
        let gone = || AppError::Refused("that concept is gone".into());
        let concept = self.store.concept(concept_id)?.ok_or_else(gone)?;
        let outcome = self.store.outcome(&concept.outcome_id)?.ok_or_else(gone)?;
        let topic = self.store.topic(&outcome.topic_id)?.ok_or_else(gone)?;
        let held = self.held_sources(concept_id, &concept.statement)?;
        if held.len() < MIN_SOURCES {
            let why = format!(
                "kizn has {} source it could read and check for this; a lecture needs at least {MIN_SOURCES}",
                held.len()
            );
            return self.save_lecture(concept_id, "unbacked", None, Some(&why));
        }
        // What the bank's questions on it say people get wrong (to take on),
        // and their wording (not to give away).
        let items: Vec<_> = self
            .store
            .items(&outcome.id, None)?
            .into_iter()
            .filter(|i| i.concept_ids.first().map(String::as_str) == Some(concept_id))
            .collect();
        let mut misconceptions: Vec<String> = vec![];
        let mut answers: Vec<String> = vec![];
        for i in &items {
            answers.push(i.stem.clone());
            let options = i.options_json.as_deref().unwrap_or("[]");
            for o in serde_json::from_str::<Vec<ItemOption>>(options).unwrap_or_default() {
                if o.correct {
                    answers.push(o.text);
                } else if let Some(m) = o.misconception.filter(|m| !misconceptions.contains(m)) {
                    misconceptions.push(m);
                }
            }
        }
        misconceptions.truncate(6);
        let others: Vec<String> =
            self.store.concepts(&outcome.id)?.into_iter().filter(|c| c.id != concept_id).map(|c| c.statement).collect();
        let shorts: Vec<String> = held.iter().map(|h| h.short.clone()).collect();
        let ask = json!({
            "topic": topic.title, "goal": topic.goal, "outcome": outcome.statement, "concept": concept.statement,
            "other_concepts": others, "supports": self.supports(&topic.id)?, "misconceptions": misconceptions,
            "sources": held.iter().map(|h| json!({"id": h.short, "title": h.title, "excerpts": h.excerpts})).collect::<Vec<_>>(),
        });

        let author = self.role_model(Role::Author)?;
        let checker = self.role_model(Role::Checker)?;
        let write = |feedback: Option<String>| {
            let mut input = ask.clone();
            if let Some(f) = feedback {
                input["feedback"] = json!(f);
            }
            let author = &author;
            async move {
                let (reply, _) = author
                    .client
                    .complete_json(
                        WRITE_LECTURE,
                        &format!("<input>\n{input}\n</input>"),
                        &lecture_schema(),
                        author.mode,
                    )
                    .await
                    .map_err(|e| AppError::Refused(format!("the author model couldn't write the lecture: {e}")))?;
                Ok::<Lecture, AppError>(serde_json::from_value(reply).unwrap_or_default())
            }
        };

        // Written, and checked in code; one more try, told why, if it fails.
        let mut lecture = write(None).await?;
        if let Err(why) = check_lecture(&lecture, &shorts, &answers) {
            lecture =
                write(Some(format!("Your last lecture was refused: {why}. Write it again, fixing that."))).await?;
            if let Err(why) = check_lecture(&lecture, &shorts, &answers) {
                return self.save_lecture(concept_id, "failed", None, Some(&format!("the lecture was refused: {why}")));
            }
        }

        // Verified against what each part cites; one rewrite of what isn't
        // supported, then whatever still isn't is left out.
        let excerpts: HashMap<&str, &Vec<String>> = held.iter().map(|h| (h.short.as_str(), &h.excerpts)).collect();
        let mut unsupported = self.verify(&checker, &concept.statement, &lecture, &excerpts).await?;
        if !unsupported.is_empty() {
            let list: Vec<String> = unsupported.iter().map(|(id, why)| format!("{id}: {why}")).collect();
            let feedback = format!(
                "These parts aren't supported by the excerpts they cite: {}. Rewrite the lecture so every part is \
                 supported by what it cites (fix, re-cite or remove those parts). Your last lecture: {}",
                list.join("; "),
                serde_json::to_string(&lecture).unwrap_or_default()
            );
            let rewritten = write(Some(feedback)).await?;
            if check_lecture(&rewritten, &shorts, &answers).is_ok() {
                lecture = rewritten;
                unsupported = self.verify(&checker, &concept.statement, &lecture, &excerpts).await?;
            }
        }
        let gaps: HashSet<String> = unsupported.into_keys().collect();
        let kept = lecture.without(&gaps);
        if let Err(why) = check_lecture(&kept, &shorts, &answers) {
            let why = format!("what its sources back is too little to read from yet ({why})");
            return self.save_lecture(concept_id, "unbacked", None, Some(&why));
        }
        let ids: HashMap<&str, &str> = held.iter().map(|h| (h.short.as_str(), h.source_id.as_str())).collect();
        let stored = kept.map_cites(|c| ids.get(c).map(|s| s.to_string()));
        self.save_lecture(concept_id, "ready", Some(&stored), None)
    }

    /// The parts the checker finds unsupported by what they cite, and why.
    /// A part it says nothing about counts as unsupported.
    async fn verify(
        &self,
        checker: &crate::topics::RoleModel,
        concept: &str,
        lecture: &Lecture,
        excerpts: &HashMap<&str, &Vec<String>>,
    ) -> Result<HashMap<String, String>, AppError> {
        let claims = lecture.claims();
        let ask = json!({
            "concept": concept,
            "claims": claims.iter().map(|c| json!({
                "id": c.id,
                "text": c.text,
                "excerpts": c.cites.iter().filter_map(|s| excerpts.get(s.as_str())).flat_map(|e| e.iter()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        let (reply, _) = checker
            .client
            .complete_json(VERIFY_LECTURE, &format!("<input>\n{ask}\n</input>"), &verdicts_schema(), checker.mode)
            .await
            .map_err(|e| AppError::Refused(format!("the checker couldn't verify the lecture: {e}")))?;
        let verdicts: HashMap<String, (bool, String)> = reply["verdicts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        Some((
                            v["id"].as_str()?.to_string(),
                            (v["supported"].as_bool().unwrap_or(false), v["why"].as_str().unwrap_or("").to_string()),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(claims
            .into_iter()
            .filter_map(|c| match verdicts.get(&c.id) {
                Some((true, _)) => None,
                Some((false, why)) => Some((c.id, why.clone())),
                None => Some((c.id, "the checker didn't say".into())),
            })
            .collect())
    }

    /// A concept's lecture for the page: its state, and the lecture with
    /// numbered footnotes once it's ready.
    pub(crate) fn lecture_view(
        &self,
        concept_id: &str,
    ) -> Result<(String, Option<LectureView>, Option<String>), AppError> {
        if self.lecturing.lock().unwrap().contains(concept_id) {
            return Ok(("writing".into(), None, None));
        }
        let Some(row) = self.store.lecture(concept_id)? else {
            return Ok(("none".into(), None, None));
        };
        let body: Option<Lecture> = row.body_json.as_deref().and_then(|b| serde_json::from_str(b).ok());
        let Some(body) = body.filter(|_| row.status == "ready") else {
            return Ok((row.status, None, row.reason));
        };
        let sources: HashMap<String, kizn_store::SourceRow> =
            self.store.sources(concept_id)?.into_iter().map(|s| (s.id.clone(), s)).collect();
        let order = body.footnotes();
        let n = |cites: &[String]| -> Vec<u32> {
            cites.iter().filter_map(|c| order.iter().position(|o| o == c)).map(|i| i as u32 + 1).collect()
        };
        let view = LectureView {
            key_terms: body
                .key_terms
                .iter()
                .map(|t| TermView { term: t.term.clone(), meaning: t.meaning.clone(), notes: n(&t.cites) })
                .collect(),
            sections: body
                .sections
                .iter()
                .map(|s| SectionView {
                    heading: s.heading.clone(),
                    paragraphs: s
                        .paragraphs
                        .iter()
                        .map(|p| ParagraphView { text: p.text.clone(), notes: n(&p.cites) })
                        .collect(),
                })
                .collect(),
            misconceptions: body
                .misconceptions
                .iter()
                .map(|m| MixUpView { belief: m.belief.clone(), correction: m.correction.clone(), notes: n(&m.cites) })
                .collect(),
            footnotes: order
                .iter()
                .enumerate()
                .filter_map(|(i, id)| {
                    sources.get(id).map(|s| FootnoteView { n: i as u32 + 1, source: crate::study::source_view(s) })
                })
                .collect(),
        };
        Ok(("ready".into(), Some(view), None))
    }
}
