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
You write a lecture for one adult learner on ONE concept, to read when they need it (often right after missing a question on it). Teach it the way a good teacher would for what they're learning and why: what it is, how it works, why it matters for their goal, a concrete example, and where people go wrong. Pitch it at the level their goal and outcome need: build intuition before detail, and leave out implementation trivia their goal doesn't call for.

Rules:
- \"title\": a short title for the reading (a few words), not the concept's whole statement.
- 3 to 6 key terms, each with a plain meaning.
- 2 to 4 sections, each with 1 to 3 short paragraphs. Each paragraph makes one or two clear, checkable points: kizn backs every part with sources afterwards, and leaves out what it can't back, so stick to what's well established.
- In \"misconceptions\", take on the mistaken beliefs listed under \"learner_mixups\" (what this learner got wrong): the belief, then what's true and why. None listed: the one or two mistakes people most often make about it.
- Don't name sources in the text, don't write practice questions, and don't echo answer options.
- If \"supports\" lists topics, explain the concept in its own terms; one of them may appear as one example among others.

Reply with JSON only: {\"title\": \"...\", \"key_terms\": [{\"term\": \"...\", \"meaning\": \"...\"}], \"sections\": [{\"heading\": \"...\", \"paragraphs\": [{\"text\": \"...\"}]}], \"misconceptions\": [{\"belief\": \"...\", \"correction\": \"...\"}]}";

const BACK_CLAIMS: &str = "kizn-task: back-claims
For each claim, say which of its candidate passages back it: every fact in the claim is stated in, or follows directly from, those passages. Rephrasing or explaining backed facts is fine; a fact no passage contains is not backed, however well known. List the sources (\"s1\"…) of the passages that back it.

Reply with JSON only: {\"verdicts\": [{\"id\": \"p0.0\", \"supported\": true, \"sources\": [\"s1\"], \"why\": \"...\"}]}";

fn lecture_schema() -> Value {
    let item = |props: Value, required: Value| json!({"type": "object", "properties": props, "required": required, "additionalProperties": false});
    json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "key_terms": {"type": "array", "items": item(
                json!({"term": {"type": "string"}, "meaning": {"type": "string"}}), json!(["term", "meaning"]))},
            "sections": {"type": "array", "items": item(
                json!({"heading": {"type": "string"}, "paragraphs": {"type": "array", "items": item(
                    json!({"text": {"type": "string"}}), json!(["text"]))}}),
                json!(["heading", "paragraphs"]))},
            "misconceptions": {"type": "array", "items": item(
                json!({"belief": {"type": "string"}, "correction": {"type": "string"}}), json!(["belief", "correction"]))}
        },
        "required": ["title", "key_terms", "sections", "misconceptions"], "additionalProperties": false
    })
}

fn verdicts_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"verdicts": {"type": "array", "items": {
            "type": "object",
            "properties": {
                "id": {"type": "string"}, "supported": {"type": "boolean"},
                "sources": {"type": "array", "items": {"type": "string"}}, "why": {"type": "string"}
            },
            "required": ["id", "supported", "sources", "why"], "additionalProperties": false
        }}},
        "required": ["verdicts"], "additionalProperties": false
    })
}

/// A source a lecture can cite: a short id for the checker, and its text.
struct Held {
    short: String,
    source_id: String,
    kind: String,
    content: String,
}

/// A claim's candidate passages: the best match for it from each source.
fn candidates(held: &[Held], claim: &str) -> Vec<(String, String)> {
    held.iter()
        .flat_map(|h| {
            crate::study::excerpts_of(&h.kind, &h.content, claim, 2).into_iter().map(|t| (h.short.clone(), t))
        })
        .collect()
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

    /// The sources a lecture can cite: verified, with text kizn holds.
    fn held_sources(&self, concept_id: &str) -> Result<Vec<Held>, AppError> {
        let mut out = vec![];
        for s in self.store.sources(concept_id)? {
            let Some(content) = s.content_json.filter(|_| s.status == "verified") else {
                continue;
            };
            out.push(Held { short: format!("s{}", out.len() + 1), source_id: s.id, kind: s.kind, content });
        }
        Ok(out)
    }

    /// Teaches first, backs second: the author writes the lecture from what
    /// it knows, for the learner's goal; then each part is matched with
    /// passages from the sources kizn read, and the checker says which back
    /// it. What nothing backs gets one rewrite toward what the sources say,
    /// then is left out.
    async fn write_lecture(&self, concept_id: &str) -> Result<(), AppError> {
        let gone = || AppError::Refused("that concept is gone".into());
        let concept = self.store.concept(concept_id)?.ok_or_else(gone)?;
        let outcome = self.store.outcome(&concept.outcome_id)?.ok_or_else(gone)?;
        let topic = self.store.topic(&outcome.topic_id)?.ok_or_else(gone)?;
        let held = self.held_sources(concept_id)?;
        if held.len() < MIN_SOURCES {
            let why = format!(
                "kizn has {} source it could read and trust for this; a lecture needs at least {MIN_SOURCES}",
                held.len()
            );
            return self.save_lecture(concept_id, "unbacked", None, Some(&why));
        }
        // The bank's wording (not to give away), and the mistakes this
        // learner made on it: the misconceptions behind the options they
        // picked when wrong.
        let items: Vec<_> = self
            .store
            .items(&outcome.id, None)?
            .into_iter()
            .filter(|i| i.concept_ids.first().map(String::as_str) == Some(concept_id))
            .collect();
        let attempts = self.store.attempts(Some(&topic.id))?;
        let mut answers: Vec<String> = vec![];
        let mut learner_mixups: Vec<String> = vec![];
        for i in &items {
            answers.push(i.stem.clone());
            let options: Vec<ItemOption> =
                serde_json::from_str(i.options_json.as_deref().unwrap_or("[]")).unwrap_or_default();
            answers.extend(options.iter().filter(|o| o.correct).map(|o| o.text.clone()));
            for a in attempts.iter().filter(|a| a.item_id == i.id && !a.correct && a.disputed_at.is_none()) {
                if let Some(m) = options.get(a.chosen).and_then(|o| o.misconception.clone())
                    && !learner_mixups.contains(&m)
                {
                    learner_mixups.push(m);
                }
            }
        }
        learner_mixups.truncate(3);
        let others: Vec<String> =
            self.store.concepts(&outcome.id)?.into_iter().filter(|c| c.id != concept_id).map(|c| c.statement).collect();
        let ask = json!({
            "topic": topic.title, "goal": topic.goal, "outcome": outcome.statement, "tier": outcome.target_tier,
            "concept": concept.statement, "other_concepts": others, "supports": self.supports(&topic.id)?,
            "learner_mixups": learner_mixups,
        });

        let author = self.role_model(Role::Author)?;
        let writer = author.client.clone().with_max_tokens(8000);
        let checker = self.role_model(Role::Checker)?;
        let write = |feedback: Option<Value>| {
            let mut input = ask.clone();
            if let Some(f) = feedback {
                input["feedback"] = f;
            }
            let (writer, mode) = (&writer, author.mode);
            async move {
                let (reply, _) = writer
                    .complete_json(WRITE_LECTURE, &format!("<input>\n{input}\n</input>"), &lecture_schema(), mode)
                    .await
                    .map_err(|e| AppError::Refused(format!("the author model couldn't write the lecture: {e}")))?;
                Ok::<Lecture, AppError>(serde_json::from_value(reply).unwrap_or_default())
            }
        };

        let shorts: Vec<String> = held.iter().map(|h| h.short.clone()).collect();
        let lecture = write(None).await?;
        let (mut backed, mut unbacked) = self.back(&checker, &concept.statement, &lecture, &held).await?;
        if !unbacked.is_empty() {
            // One rewrite: what couldn't be backed, with what the sources do say.
            let gaps: Vec<Value> = lecture
                .claims()
                .into_iter()
                .filter(|c| unbacked.contains_key(&c.id))
                .map(|c| {
                    json!({"part": c.id, "text": c.text, "why": unbacked[&c.id],
                           "passages": candidates(&held, &c.text).into_iter().map(|(_, t)| t).collect::<Vec<_>>()})
                })
                .collect();
            let feedback = json!({
                "note": "These parts couldn't be backed by the sources kizn read. Rewrite the lecture so they say \
                         what these passages support, or leave them out; keep the rest.",
                "unbacked": gaps,
                "last_lecture": lecture,
            });
            let rewritten = write(Some(feedback)).await?;
            let (b, u) = self.back(&checker, &concept.statement, &rewritten, &held).await?;
            if u.len() <= unbacked.len() {
                (backed, unbacked) = (b, u);
            }
        }
        let gaps: HashSet<String> = unbacked.into_keys().collect();
        let kept = backed.without(&gaps);
        if let Err(why) = check_lecture(&kept, &shorts, &answers, &concept.statement) {
            let why = format!("what its sources back is too little to read from yet ({why})");
            return self.save_lecture(concept_id, "unbacked", None, Some(&why));
        }
        let ids: HashMap<&str, &str> = held.iter().map(|h| (h.short.as_str(), h.source_id.as_str())).collect();
        let stored = kept.map_cites(|c| ids.get(c).map(|s| s.to_string()));
        self.save_lecture(concept_id, "ready", Some(&stored), None)
    }

    /// Each part of a lecture matched with candidate passages and judged by
    /// the checker: the lecture with every backed part citing its sources,
    /// and the parts nothing backs, with why. A part the checker says nothing
    /// about isn't backed.
    async fn back(
        &self,
        checker: &crate::topics::RoleModel,
        concept: &str,
        lecture: &Lecture,
        held: &[Held],
    ) -> Result<(Lecture, HashMap<String, String>), AppError> {
        let claims = lecture.claims();
        let offered: HashMap<String, Vec<(String, String)>> =
            claims.iter().map(|c| (c.id.clone(), candidates(held, &c.text))).collect();
        let ask = json!({
            "concept": concept,
            "claims": claims.iter().map(|c| json!({
                "id": c.id,
                "text": c.text,
                "candidates": offered[&c.id].iter().map(|(s, t)| json!({"source": s, "text": t})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        let (reply, _) = checker
            .client
            .clone()
            .with_max_tokens(8000)
            .complete_json(BACK_CLAIMS, &format!("<input>\n{ask}\n</input>"), &verdicts_schema(), checker.mode)
            .await
            .map_err(|e| AppError::Refused(format!("the checker couldn't back the lecture: {e}")))?;
        let mut cites: HashMap<String, Vec<String>> = HashMap::new();
        let mut why: HashMap<String, String> = HashMap::new();
        for v in reply["verdicts"].as_array().into_iter().flatten() {
            let Some(id) = v["id"].as_str() else { continue };
            let Some(candidate_sources) = offered.get(id) else { continue };
            // Only sources whose passages it was shown count.
            let mut sources: Vec<String> = v["sources"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str())
                .filter(|s| candidate_sources.iter().any(|(c, _)| c == s))
                .map(str::to_string)
                .collect();
            sources.dedup();
            if v["supported"].as_bool() == Some(true) && !sources.is_empty() {
                cites.insert(id.to_string(), sources);
            } else {
                why.insert(id.to_string(), v["why"].as_str().unwrap_or("not backed").to_string());
            }
        }
        for c in &claims {
            if !cites.contains_key(&c.id) && !why.contains_key(&c.id) {
                why.insert(c.id.clone(), "the checker didn't say".into());
            }
        }
        Ok((lecture.with_cites(&cites), why))
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
            title: body.title.clone(),
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
