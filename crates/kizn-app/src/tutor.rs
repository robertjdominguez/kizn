//! The anchored tutor: a conversation that starts from a thing (a card, a
//! concept, later a resource or a passage in it), never from a blank box. The
//! tutor talks; it can't grade, record or move anything: nothing here touches
//! attempts, and concept states and recalls come only from those.

use futures_util::StreamExt;
use kizn_model::Turn;
use kizn_store::{MessageRow, ThreadRow};
use serde_json::{Value, json};

use crate::command::{Anchor, Event, MessageView, Role, ThreadSummary, ThreadView};
use crate::{AppError, Ctx};

const LETTERS: [&str; 6] = ["A", "B", "C", "D", "E", "F"];

const TUTOR: &str = "kizn-task: tutor
You are kizn's tutor for one adult learner. The conversation is about one thing they're studying, given in <input>: a practice question they answered (with the option they picked, the key and why each option is right or wrong), a concept, or a resource (a page or a video) with excerpts from it, and sometimes a passage or moment they picked out.

- Help them understand. Ground what you say in the thing in <input>; when you use a resource or excerpt given there, name it (and the moment, for a video), and when it's your own knowledge, say so.
- Explain rather than quiz. Ask them a question back only if they ask to be tested.
- Plain language, short: under about 150 words unless they ask for more. Use Markdown sparingly (short lists, `code`).
- You can't grade, score, record or change anything: not their answers, not whether a key is right, not when things come back. kizn keeps score from their answers alone. If they ask you to change any of it, say you can't and why. If they think a question's key is wrong, tell them they can say so with \"This key is wrong\" on the card.";

impl Anchor {
    fn kind(&self) -> &'static str {
        match self {
            Anchor::Card { .. } => "card",
            Anchor::Concept { .. } => "concept",
            Anchor::Resource { .. } => "resource",
        }
    }

    fn id(&self) -> &str {
        match self {
            Anchor::Card { item_id } => item_id,
            Anchor::Concept { concept_id } => concept_id,
            Anchor::Resource { source_id } => source_id,
        }
    }

    fn from_row(kind: &str, id: &str) -> Anchor {
        match kind {
            "concept" => Anchor::Concept { concept_id: id.into() },
            "resource" => Anchor::Resource { source_id: id.into() },
            _ => Anchor::Card { item_id: id.into() },
        }
    }
}

impl Ctx {
    /// What the tutor is given about the anchor, and the topic it's in.
    fn anchor_context(&self, anchor: &Anchor, query: &str) -> Result<(String, Value), AppError> {
        match anchor {
            Anchor::Card { item_id } => {
                let item =
                    self.store.item(item_id)?.ok_or_else(|| AppError::Refused("there's no such question".into()))?;
                let options: Vec<kizn_core::bank::ItemOption> =
                    serde_json::from_str(item.options_json.as_deref().unwrap_or("[]")).unwrap_or_default();
                // As the learner saw it: their last answer's order and pick.
                let last = self.store.attempts(Some(&item.topic_id))?.into_iter().rfind(|a| a.item_id == *item_id);
                let order: Vec<usize> =
                    last.as_ref().map(|a| a.order.clone()).unwrap_or_else(|| (0..options.len()).collect());
                let shown: Vec<Value> = order
                    .iter()
                    .enumerate()
                    .filter_map(|(pos, &i)| {
                        options.get(i).map(|o| {
                            json!({ "letter": LETTERS.get(pos).copied().unwrap_or("?"), "text": o.text, "correct": o.correct,
                                    "why": o.why, "misconception": o.misconception })
                        })
                    })
                    .collect();
                let letter = |stored: usize| {
                    order.iter().position(|&i| i == stored).and_then(|p| LETTERS.get(p).copied()).unwrap_or("?")
                };
                let key = options.iter().position(|o| o.correct).unwrap_or(0);
                let concepts: Vec<String> = item
                    .concept_ids
                    .iter()
                    .filter_map(|c| self.store.concept(c).ok().flatten().map(|c| c.statement))
                    .collect();
                Ok((
                    item.topic_id.clone(),
                    json!({
                        "kind": "card",
                        "question": item.stem,
                        "options": shown,
                        "picked": last.as_ref().map(|a| format!("{} ({})", letter(a.chosen), options.get(a.chosen).map(|o| o.text.as_str()).unwrap_or(""))),
                        "picked_correct": last.as_ref().map(|a| a.correct),
                        "key": format!("{} ({})", letter(key), options.get(key).map(|o| o.text.as_str()).unwrap_or("")),
                        "key_why": options.get(key).map(|o| o.why.as_str()).unwrap_or(""),
                        "concepts": concepts,
                        "source": item.source_ref,
                        "model_knowledge": item.model_knowledge,
                    }),
                ))
            }
            Anchor::Concept { concept_id } => {
                let c = self
                    .store
                    .concept(concept_id)?
                    .ok_or_else(|| AppError::Refused("there's no such concept".into()))?;
                let o = self
                    .store
                    .outcome(&c.outcome_id)?
                    .ok_or_else(|| AppError::Refused("its outcome is gone".into()))?;
                // The tutor can reach the concept's resources: the excerpts
                // from each that best match the question.
                let q = format!("{query} {}", c.statement);
                let sources: Vec<Value> = self
                    .store
                    .sources(concept_id)?
                    .into_iter()
                    .filter(|s| s.status == "verified")
                    .map(|s| json!({ "title": s.title, "kind": s.kind, "url": s.url, "excerpts": self.excerpts(&s, &q, 2) }))
                    .collect();
                Ok((
                    o.topic_id.clone(),
                    json!({ "kind": "concept", "title": c.statement, "concept": c.statement, "outcome": o.statement,
                            "resources": sources, "note": self.store.note(concept_id)? }),
                ))
            }
            Anchor::Resource { source_id } => {
                let s = self
                    .store
                    .source(source_id)?
                    .ok_or_else(|| AppError::Refused("there's no such resource".into()))?;
                if s.status != "verified" || s.content_json.is_none() {
                    return Err(AppError::Refused("kizn doesn't hold that resource's text yet: open it first".into()));
                }
                let c = self
                    .store
                    .concept(&s.concept_id)?
                    .ok_or_else(|| AppError::Refused("its concept is gone".into()))?;
                let o = self
                    .store
                    .outcome(&c.outcome_id)?
                    .ok_or_else(|| AppError::Refused("its outcome is gone".into()))?;
                let q = format!("{query} {}", c.statement);
                Ok((
                    o.topic_id.clone(),
                    json!({ "kind": "resource", "title": s.title, "resource_kind": s.kind, "url": s.url,
                            "concept": c.statement, "excerpts": self.excerpts(&s, &q, 4),
                            "automatic_captions": s.automatic }),
                ))
            }
        }
    }

    /// A resource's stretches that best match the question (the tutor's
    /// excerpts), with where each is.
    fn excerpts(&self, s: &kizn_store::SourceRow, query: &str, k: usize) -> Vec<Value> {
        let content = s.content_json.as_deref().unwrap_or("[]");
        let chunks = if s.kind == "video" {
            let cues: Vec<kizn_core::source::Cue> = serde_json::from_str(content).unwrap_or_default();
            kizn_core::source::cue_chunks(&cues, 60.0)
        } else {
            let blocks: Vec<kizn_core::source::Block> = serde_json::from_str(content).unwrap_or_default();
            kizn_core::source::page_chunks(&blocks, 120)
        };
        kizn_core::source::relevant(&chunks, query, k)
            .into_iter()
            .map(|c| if c.at.is_empty() { json!({ "text": c.text }) } else { json!({ "at": c.at, "text": c.text }) })
            .collect()
    }

    fn thread_view(&self, t: &ThreadRow) -> Result<ThreadView, AppError> {
        let messages =
            self.store.messages(&t.id)?.into_iter().map(|m| MessageView { role: m.role, text: m.text }).collect();
        Ok(ThreadView {
            id: t.id.clone(),
            title: t.title.clone(),
            anchor: Anchor::from_row(&t.anchor_kind, &t.anchor_id),
            messages,
            streaming: self.tutoring.lock().unwrap().contains_key(&t.id),
        })
    }

    pub(crate) fn threads(&self, anchor: &Anchor) -> Result<Vec<ThreadSummary>, AppError> {
        self.store
            .threads(anchor.kind(), anchor.id())?
            .into_iter()
            .map(|t| {
                let messages = self.store.messages(&t.id)?.len() as i64;
                Ok(ThreadSummary { id: t.id, title: t.title, messages, updated_at: t.updated_at })
            })
            .collect()
    }

    pub(crate) fn get_thread(&self, thread_id: &str) -> Result<ThreadView, AppError> {
        let t =
            self.store.thread(thread_id)?.ok_or_else(|| AppError::Refused("there's no such conversation".into()))?;
        self.thread_view(&t)
    }

    /// The learner asks; the reply streams as TutorDelta events and is kept
    /// when it's done (TutorDone). A new conversation unless `thread_id`.
    pub(crate) fn ask(
        &self,
        anchor: &Anchor,
        thread_id: Option<&str>,
        text: &str,
        passage: Option<&str>,
    ) -> Result<ThreadView, AppError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(AppError::Refused("ask something first".into()));
        }
        let passage = passage.map(str::trim).filter(|p| !p.is_empty());
        let (topic_id, context) = self.anchor_context(anchor, &format!("{text} {}", passage.unwrap_or("")))?;
        let tutor = self.role_model(Role::Tutor).or_else(|_| self.role_model(Role::Author))?;
        let thread = match thread_id {
            Some(id) => {
                self.store.thread(id)?.ok_or_else(|| AppError::Refused("there's no such conversation".into()))?
            }
            None => {
                let now = self.now();
                let t = ThreadRow {
                    id: self.id(),
                    topic_id,
                    anchor_kind: anchor.kind().into(),
                    anchor_id: anchor.id().into(),
                    title: text.chars().take(80).collect(),
                    created_at: now.clone(),
                    updated_at: now,
                };
                self.store.insert_thread(&t)?;
                t
            }
        };
        if self.tutoring.lock().unwrap().contains_key(&thread.id) {
            return Err(AppError::Refused("the tutor is still answering".into()));
        }
        // The conversation so far, then this question with the anchor.
        let mut turns: Vec<(Turn, String)> = self
            .store
            .messages(&thread.id)?
            .into_iter()
            .map(|m| (if m.role == "tutor" { Turn::Assistant } else { Turn::User }, m.text))
            .collect();
        turns.push((
            Turn::User,
            format!(
                "{text}\n\n<input>\n{}\n</input>",
                json!({ "anchor": context, "question": text, "passage": passage })
            ),
        ));
        // Kept with the passage quoted, so the conversation reads back right.
        let said = match passage {
            Some(p) => format!("> {}\n\n{text}", p.replace('\n', " ")),
            None => text.to_string(),
        };
        self.store.add_message(&MessageRow {
            id: self.id(),
            thread_id: thread.id.clone(),
            role: "learner".into(),
            text: said,
            at: self.now(),
        })?;
        self.tutoring.lock().unwrap().insert(thread.id.clone(), tutor.provider.clone());
        let ctx = self.clone();
        let id = thread.id.clone();
        tokio::spawn(async move {
            let mut reply = String::new();
            let mut failed: Option<String> = None;
            match tutor.client.stream_chat(TUTOR, &turns).await {
                Err(e) => failed = Some(e.to_string()),
                Ok(stream) => {
                    let mut stream = std::pin::pin!(stream);
                    while let Some(delta) = stream.next().await {
                        match delta {
                            Ok(d) => {
                                reply.push_str(&d);
                                let _ = ctx.events.send(Event::TutorDelta { thread_id: id.clone(), text: d });
                            }
                            Err(e) => {
                                failed = Some(e.to_string());
                                break;
                            }
                        }
                    }
                }
            }
            if !reply.trim().is_empty() {
                let _ = ctx.store.add_message(&MessageRow {
                    id: ctx.id(),
                    thread_id: id.clone(),
                    role: "tutor".into(),
                    text: reply,
                    at: ctx.now(),
                });
            }
            // Announced while still counted as answering, so the bank writer
            // (which waits for the tutor) only starts once it's done.
            let _ = ctx.events.send(Event::TutorDone { thread_id: id.clone(), error: failed });
            ctx.tutoring.lock().unwrap().remove(&id);
        });
        self.thread_view(&thread)
    }
}
