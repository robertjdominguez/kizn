//! Topics, their outcomes and concepts, and the item bank. Plain rows: what
//! they mean (states, gates) is computed in kizn-core and kizn-app.

use rusqlite::{OptionalExtension, Row, params};

use crate::{Result, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRow {
    pub id: String,
    pub title: String,
    pub goal: String,
    /// draft | confirmed
    pub status: String,
    /// drafting | drafted | stopped | failed
    pub draft_state: String,
    pub draft_error: Option<String>,
    pub created_at: String,
    pub confirmed_at: Option<String>,
    /// "learner", or "kizn" for a foundation it prepared by itself.
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeRow {
    pub id: String,
    pub topic_id: String,
    pub ord: i64,
    pub statement: String,
    pub target_tier: String,
    pub tier_reason: String,
    /// pending | drafting | done
    pub concepts_state: String,
    /// waiting | writing | ready | failed
    pub bank_state: String,
    pub bank_error: Option<String>,
    pub retry_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptRow {
    pub id: String,
    pub outcome_id: String,
    pub ord: i64,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    pub id: String,
    pub topic_id: String,
    pub outcome_id: String,
    pub batch_id: Option<String>,
    pub kind: String,
    pub tier: String,
    pub concept_ids: Vec<String>,
    pub stem: String,
    pub options_json: Option<String>,
    pub exemplar: Option<String>,
    pub source_ref: Option<String>,
    pub model_knowledge: bool,
    /// active | refused | retired
    pub status: String,
    pub status_reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRow {
    pub id: String,
    pub topic_id: String,
    pub outcome_id: String,
    pub item_id: String,
    /// The stored options' indices, in the order they were shown.
    pub order: Vec<usize>,
    /// Index into the stored options.
    pub chosen: usize,
    pub correct: bool,
    pub at: String,
    pub disputed_at: Option<String>,
}

/// A concept's lecture, and how far it's got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LectureRow {
    pub concept_id: String,
    /// writing | ready | unbacked | failed
    pub status: String,
    pub body_json: Option<String>,
    pub reason: Option<String>,
    pub written_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRow {
    pub id: String,
    pub upper_id: String,
    pub lower_id: String,
    /// rests_on | part_of
    pub kind: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionRow {
    pub id: String,
    pub for_topic: String,
    pub title: String,
    pub why: Option<String>,
    /// draft | check | flag
    pub source: String,
    /// open | kept | dropped | later | started | linked | dismissed
    pub status: String,
    /// The topic it is (one the learner has, or one started from it).
    pub topic_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundItemRow {
    pub id: String,
    pub suggestion_id: String,
    pub stem: String,
    pub options_json: String,
    /// The learner's answer: the stored option chosen and whether it's right.
    pub answer: Option<(usize, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    pub id: String,
    pub topic_id: String,
    pub anchor_kind: String,
    pub anchor_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    pub id: String,
    pub thread_id: String,
    /// learner | tutor
    pub role: String,
    pub text: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub id: String,
    pub concept_id: String,
    pub url: String,
    pub title: String,
    pub why: Option<String>,
    /// verified | rejected | failed
    pub status: String,
    pub reason: Option<String>,
    pub words: Option<i64>,
    pub checked_at: String,
    /// page | video
    pub kind: String,
    /// The text kept for reading: [Block] for a page, [Cue] for a video.
    pub content_json: Option<String>,
    /// A video's captions were generated, not written by a person.
    pub automatic: bool,
    /// kizn | learner
    pub added_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchCounts {
    pub asked: i64,
    pub accepted: i64,
    pub refused: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

const TOPIC_COLS: &str = "id, title, goal_md, status, draft_state, draft_error, created_at, confirmed_at, origin";
const OUTCOME_COLS: &str =
    "id, topic_id, ord, statement, target_tier, tier_reason, concepts_state, bank_state, bank_error, retry_at";
const ITEM_COLS: &str = "id, topic_id, outcome_id, batch_id, kind, tier, concept_ids_json, stem, options_json, exemplar, source_ref, model_knowledge, status, status_reason, created_at";

fn topic(r: &Row) -> rusqlite::Result<TopicRow> {
    Ok(TopicRow {
        id: r.get(0)?,
        title: r.get(1)?,
        goal: r.get(2)?,
        status: r.get(3)?,
        draft_state: r.get(4)?,
        draft_error: r.get(5)?,
        created_at: r.get(6)?,
        confirmed_at: r.get(7)?,
        origin: r.get(8)?,
    })
}

fn outcome(r: &Row) -> rusqlite::Result<OutcomeRow> {
    Ok(OutcomeRow {
        id: r.get(0)?,
        topic_id: r.get(1)?,
        ord: r.get(2)?,
        statement: r.get(3)?,
        target_tier: r.get(4)?,
        tier_reason: r.get(5)?,
        concepts_state: r.get(6)?,
        bank_state: r.get(7)?,
        bank_error: r.get(8)?,
        retry_at: r.get(9)?,
    })
}

fn item(r: &Row) -> rusqlite::Result<ItemRow> {
    let ids: String = r.get(6)?;
    Ok(ItemRow {
        id: r.get(0)?,
        topic_id: r.get(1)?,
        outcome_id: r.get(2)?,
        batch_id: r.get(3)?,
        kind: r.get(4)?,
        tier: r.get(5)?,
        concept_ids: serde_json::from_str(&ids).unwrap_or_default(),
        stem: r.get(7)?,
        options_json: r.get(8)?,
        exemplar: r.get(9)?,
        source_ref: r.get(10)?,
        model_knowledge: r.get::<_, i64>(11)? != 0,
        status: r.get(12)?,
        status_reason: r.get(13)?,
        created_at: r.get(14)?,
    })
}

impl Store {
    // ---- topics ----

    pub fn create_draft_topic(&self, id: &str, title: &str, goal: &str, now: &str) -> Result<()> {
        self.create_topic_from(id, title, goal, now, "learner")
    }

    /// A draft set up by `origin` ("learner" or "kizn").
    pub fn create_topic_from(&self, id: &str, title: &str, goal: &str, now: &str, origin: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO topics (id, title, goal_md, created_at, status, draft_state, origin) VALUES (?1, ?2, ?3, ?4, 'draft', 'drafting', ?5)",
            params![id, title, goal, now, origin],
        )?;
        Ok(())
    }

    pub fn topic(&self, id: &str) -> Result<Option<TopicRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(&format!("SELECT {TOPIC_COLS} FROM topics WHERE id = ?1"), [id], topic).optional()?)
    }

    pub fn all_topics(&self) -> Result<Vec<TopicRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!("SELECT {TOPIC_COLS} FROM topics ORDER BY created_at, id"))?;
        Ok(stmt.query_map([], topic)?.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_draft_state(&self, topic_id: &str, state: &str, error: Option<&str>) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE topics SET draft_state = ?2, draft_error = ?3 WHERE id = ?1",
            params![topic_id, state, error],
        )?;
        Ok(())
    }

    pub fn confirm_topic(&self, topic_id: &str, now: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE topics SET status = 'confirmed', confirmed_at = ?2 WHERE id = ?1",
            params![topic_id, now],
        )?;
        Ok(())
    }

    // ---- outcomes and concepts ----

    pub fn insert_outcome(&self, o: &OutcomeRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            &format!("INSERT INTO outcomes ({OUTCOME_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"),
            params![
                o.id,
                o.topic_id,
                o.ord,
                o.statement,
                o.target_tier,
                o.tier_reason,
                o.concepts_state,
                o.bank_state,
                o.bank_error,
                o.retry_at
            ],
        )?;
        Ok(())
    }

    pub fn outcomes(&self, topic_id: &str) -> Result<Vec<OutcomeRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare(&format!("SELECT {OUTCOME_COLS} FROM outcomes WHERE topic_id = ?1 ORDER BY ord"))?;
        Ok(stmt.query_map([topic_id], outcome)?.collect::<rusqlite::Result<_>>()?)
    }

    pub fn outcome(&self, id: &str) -> Result<Option<OutcomeRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(&format!("SELECT {OUTCOME_COLS} FROM outcomes WHERE id = ?1"), [id], outcome).optional()?)
    }

    pub fn set_concepts_state(&self, outcome_id: &str, state: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE outcomes SET concepts_state = ?2 WHERE id = ?1", params![outcome_id, state])?;
        Ok(())
    }

    pub fn set_bank_state(
        &self,
        outcome_id: &str,
        state: &str,
        error: Option<&str>,
        retry_at: Option<&str>,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE outcomes SET bank_state = ?2, bank_error = ?3, retry_at = ?4 WHERE id = ?1",
            params![outcome_id, state, error, retry_at],
        )?;
        Ok(())
    }

    pub fn insert_concept(&self, c: &ConceptRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO concepts (id, outcome_id, ord, statement) VALUES (?1, ?2, ?3, ?4)",
            params![c.id, c.outcome_id, c.ord, c.statement],
        )?;
        Ok(())
    }

    pub fn concept(&self, id: &str) -> Result<Option<ConceptRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT id, outcome_id, ord, statement FROM concepts WHERE id = ?1", [id], |r| {
                Ok(ConceptRow { id: r.get(0)?, outcome_id: r.get(1)?, ord: r.get(2)?, statement: r.get(3)? })
            })
            .optional()?)
    }

    pub fn concepts(&self, outcome_id: &str) -> Result<Vec<ConceptRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, outcome_id, ord, statement FROM concepts WHERE outcome_id = ?1 ORDER BY ord")?;
        let rows = stmt.query_map([outcome_id], |r| {
            Ok(ConceptRow { id: r.get(0)?, outcome_id: r.get(1)?, ord: r.get(2)?, statement: r.get(3)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Replaces a draft's outcomes and concepts with the shaped ones, in one
    /// transaction. Only for drafts: confirmed topics aren't edited.
    pub fn replace_draft(&self, topic_id: &str, outcomes: &[(OutcomeRow, Vec<ConceptRow>)]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM outcomes WHERE topic_id = ?1", [topic_id])?;
        for (o, concepts) in outcomes {
            tx.execute(
                &format!("INSERT INTO outcomes ({OUTCOME_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"),
                params![
                    o.id,
                    topic_id,
                    o.ord,
                    o.statement,
                    o.target_tier,
                    o.tier_reason,
                    o.concepts_state,
                    o.bank_state,
                    o.bank_error,
                    o.retry_at
                ],
            )?;
            for c in concepts {
                tx.execute(
                    "INSERT INTO concepts (id, outcome_id, ord, statement) VALUES (?1, ?2, ?3, ?4)",
                    params![c.id, o.id, c.ord, c.statement],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- the bank ----

    pub fn insert_item(&self, i: &ItemRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            &format!("INSERT INTO items ({ITEM_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"),
            params![
                i.id,
                i.topic_id,
                i.outcome_id,
                i.batch_id,
                i.kind,
                i.tier,
                serde_json::to_string(&i.concept_ids).expect("ids serialise"),
                i.stem,
                i.options_json,
                i.exemplar,
                i.source_ref,
                i.model_knowledge as i64,
                i.status,
                i.status_reason,
                i.created_at
            ],
        )?;
        Ok(())
    }

    /// An outcome's items, optionally only those with `status`.
    pub fn items(&self, outcome_id: &str, status: Option<&str>) -> Result<Vec<ItemRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {ITEM_COLS} FROM items WHERE outcome_id = ?1 AND (?2 IS NULL OR status = ?2) ORDER BY created_at, id"
        ))?;
        Ok(stmt.query_map(params![outcome_id, status], item)?.collect::<rusqlite::Result<_>>()?)
    }

    /// Active items per outcome of a topic, for "N items ready".
    pub fn active_counts(&self, topic_id: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT outcome_id, count(*) FROM items WHERE topic_id = ?1 AND status = 'active' GROUP BY outcome_id",
        )?;
        Ok(stmt.query_map([topic_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?)
    }

    pub fn item(&self, id: &str) -> Result<Option<ItemRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(&format!("SELECT {ITEM_COLS} FROM items WHERE id = ?1"), [id], item).optional()?)
    }

    pub fn insert_attempt(&self, a: &AttemptRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO attempts (id, topic_id, outcome_id, item_id, order_json, chosen, correct, at, disputed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                a.id,
                a.topic_id,
                a.outcome_id,
                a.item_id,
                serde_json::to_string(&a.order).expect("an order serialises"),
                a.chosen as i64,
                a.correct as i64,
                a.at,
                a.disputed_at
            ],
        )?;
        Ok(())
    }

    /// Every answer, oldest first; or one topic's.
    pub fn attempts(&self, topic_id: Option<&str>) -> Result<Vec<AttemptRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, topic_id, outcome_id, item_id, order_json, chosen, correct, at, disputed_at FROM attempts
             WHERE ?1 IS NULL OR topic_id = ?1 ORDER BY at, id",
        )?;
        let rows = stmt.query_map([topic_id], |r| {
            let order: String = r.get(4)?;
            Ok(AttemptRow {
                id: r.get(0)?,
                topic_id: r.get(1)?,
                outcome_id: r.get(2)?,
                item_id: r.get(3)?,
                order: serde_json::from_str(&order).unwrap_or_default(),
                chosen: r.get::<_, i64>(5)? as usize,
                correct: r.get::<_, i64>(6)? != 0,
                at: r.get(7)?,
                disputed_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn insert_source(&self, s: &SourceRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO sources (id, concept_id, url, title, why, status, reason, words, checked_at, kind, content_json, automatic, added_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                s.id,
                s.concept_id,
                s.url,
                s.title,
                s.why,
                s.status,
                s.reason,
                s.words,
                s.checked_at,
                s.kind,
                s.content_json,
                s.automatic as i64,
                s.added_by
            ],
        )?;
        Ok(())
    }

    /// A concept's sources, oldest first: every one checked, whatever came of it.
    pub fn sources(&self, concept_id: &str) -> Result<Vec<SourceRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!("SELECT {SOURCE_COLS} FROM sources WHERE concept_id = ?1 ORDER BY checked_at, rowid"))?;
        let rows = stmt.query_map([concept_id], source)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn source(&self, id: &str) -> Result<Option<SourceRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(&format!("SELECT {SOURCE_COLS} FROM sources WHERE id = ?1"), [id], source).optional()?)
    }

    /// Keeps a resource's text (for one checked before kizn kept it).
    pub fn set_source_content(&self, id: &str, content_json: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE sources SET content_json = ?2 WHERE id = ?1", params![id, content_json])?;
        Ok(())
    }

    pub fn lecture(&self, concept_id: &str) -> Result<Option<LectureRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT concept_id, status, body_json, reason, written_at FROM lectures WHERE concept_id = ?1",
                [concept_id],
                |r| {
                    Ok(LectureRow {
                        concept_id: r.get(0)?,
                        status: r.get(1)?,
                        body_json: r.get(2)?,
                        reason: r.get(3)?,
                        written_at: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// A concept's lecture gone, with the sources kizn found for it (the
    /// learner's own stay), so both can be found and written afresh.
    pub fn forget_lecture(&self, concept_id: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM lectures WHERE concept_id = ?1", [concept_id])?;
        tx.execute("DELETE FROM sources WHERE concept_id = ?1 AND added_by = 'kizn'", [concept_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_lecture(&self, l: &LectureRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO lectures (concept_id, status, body_json, reason, written_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (concept_id) DO UPDATE SET status = ?2, body_json = ?3, reason = ?4, written_at = ?5",
            params![l.concept_id, l.status, l.body_json, l.reason, l.written_at],
        )?;
        Ok(())
    }

    pub fn note(&self, concept_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT text FROM notes WHERE concept_id = ?1", [concept_id], |r| r.get(0)).optional()?)
    }

    /// Saves a concept's note; an empty note is removed.
    pub fn save_note(&self, concept_id: &str, text: &str, at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if text.trim().is_empty() {
            conn.execute("DELETE FROM notes WHERE concept_id = ?1", [concept_id])?;
        } else {
            conn.execute(
                "INSERT INTO notes (concept_id, text, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (concept_id) DO UPDATE SET text = ?2, updated_at = ?3",
                params![concept_id, text, at],
            )?;
        }
        Ok(())
    }

    /// The learner says an answer's question has the wrong key: the answer
    /// stops counting and the question is retired as disputed. One
    /// transaction. Returns the item's id.
    pub fn dispute(&self, attempt_id: &str, at: &str) -> Result<Option<String>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let item: Option<String> =
            tx.query_row("SELECT item_id FROM attempts WHERE id = ?1", [attempt_id], |r| r.get(0)).optional()?;
        if let Some(item) = &item {
            tx.execute(
                "UPDATE attempts SET disputed_at = ?2 WHERE id = ?1 AND disputed_at IS NULL",
                params![attempt_id, at],
            )?;
            tx.execute(
                "UPDATE items SET status = 'disputed', status_reason = 'disputed: the learner says the key is wrong' WHERE id = ?1",
                [item],
            )?;
        }
        tx.commit()?;
        Ok(item)
    }

    pub fn insert_link(&self, l: &LinkRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO links (id, upper_id, lower_id, kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![l.id, l.upper_id, l.lower_id, l.kind, l.created_at],
        )?;
        Ok(())
    }

    pub fn delete_link(&self, id: &str) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM links WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Every link on the map.
    pub fn links(&self) -> Result<Vec<LinkRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, upper_id, lower_id, kind, created_at FROM links ORDER BY created_at, id")?;
        let rows = stmt.query_map([], |r| {
            Ok(LinkRow {
                id: r.get(0)?,
                upper_id: r.get(1)?,
                lower_id: r.get(2)?,
                kind: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn insert_suggestion(&self, s: &SuggestionRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO suggestions (id, for_topic, title, why, source, status, topic_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![s.id, s.for_topic, s.title, s.why, s.source, s.status, s.topic_id, s.created_at],
        )?;
        Ok(())
    }

    /// A topic's suggested foundations, oldest first; or every one.
    pub fn suggestions(&self, for_topic: Option<&str>) -> Result<Vec<SuggestionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, for_topic, title, why, source, status, topic_id, created_at FROM suggestions
             WHERE ?1 IS NULL OR for_topic = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = stmt.query_map([for_topic], |r| {
            Ok(SuggestionRow {
                id: r.get(0)?,
                for_topic: r.get(1)?,
                title: r.get(2)?,
                why: r.get(3)?,
                source: r.get(4)?,
                status: r.get(5)?,
                topic_id: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_suggestion(&self, id: &str, status: &str, topic_id: Option<&str>) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE suggestions SET status = ?2, topic_id = COALESCE(?3, topic_id) WHERE id = ?1",
            params![id, status, topic_id],
        )?;
        Ok(())
    }

    pub fn insert_ground_item(
        &self,
        id: &str,
        suggestion_id: &str,
        stem: &str,
        options_json: &str,
        at: &str,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO ground_items (id, suggestion_id, stem, options_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, suggestion_id, stem, options_json, at],
        )?;
        Ok(())
    }

    /// A foundation's check questions, with any answers.
    pub fn ground_items(&self, suggestion_id: &str) -> Result<Vec<GroundItemRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT g.id, g.suggestion_id, g.stem, g.options_json, a.chosen, a.correct FROM ground_items g
             LEFT JOIN ground_answers a ON a.item_id = g.id WHERE g.suggestion_id = ?1 ORDER BY g.created_at, g.rowid",
        )?;
        let rows = stmt.query_map([suggestion_id], |r| {
            let chosen: Option<i64> = r.get(4)?;
            let correct: Option<i64> = r.get(5)?;
            Ok(GroundItemRow {
                id: r.get(0)?,
                suggestion_id: r.get(1)?,
                stem: r.get(2)?,
                options_json: r.get(3)?,
                answer: chosen.zip(correct).map(|(c, ok)| (c as usize, ok != 0)),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn answer_ground(&self, item_id: &str, chosen: usize, correct: bool, at: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO ground_answers (item_id, chosen, correct, at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (item_id) DO NOTHING",
            params![item_id, chosen as i64, correct as i64, at],
        )?;
        Ok(())
    }

    pub fn insert_thread(&self, t: &ThreadRow) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO threads (id, topic_id, anchor_kind, anchor_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![t.id, t.topic_id, t.anchor_kind, t.anchor_id, t.title, t.created_at, t.updated_at],
        )?;
        Ok(())
    }

    pub fn thread(&self, id: &str) -> Result<Option<ThreadRow>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT id, topic_id, anchor_kind, anchor_id, title, created_at, updated_at FROM threads WHERE id = ?1",
                [id],
                thread,
            )
            .optional()?)
    }

    /// An anchor's conversations, newest first.
    pub fn threads(&self, anchor_kind: &str, anchor_id: &str) -> Result<Vec<ThreadRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, topic_id, anchor_kind, anchor_id, title, created_at, updated_at FROM threads
             WHERE anchor_kind = ?1 AND anchor_id = ?2 ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([anchor_kind, anchor_id], thread)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A topic's conversations, oldest first.
    pub fn threads_in_topic(&self, topic_id: &str) -> Result<Vec<ThreadRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, topic_id, anchor_kind, anchor_id, title, created_at, updated_at FROM threads
             WHERE topic_id = ?1 ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([topic_id], thread)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Adds a message and touches its thread.
    pub fn add_message(&self, m: &MessageRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (id, thread_id, role, text, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![m.id, m.thread_id, m.role, m.text, m.at],
        )?;
        conn.execute("UPDATE threads SET updated_at = ?2 WHERE id = ?1", params![m.thread_id, m.at])?;
        Ok(())
    }

    pub fn messages(&self, thread_id: &str) -> Result<Vec<MessageRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, thread_id, role, text, at FROM messages WHERE thread_id = ?1 ORDER BY at, rowid")?;
        let rows = stmt.query_map([thread_id], |r| {
            Ok(MessageRow { id: r.get(0)?, thread_id: r.get(1)?, role: r.get(2)?, text: r.get(3)?, at: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn mark_studied(&self, concept_id: &str, at: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO studied (concept_id, at) VALUES (?1, ?2) ON CONFLICT (concept_id) DO UPDATE SET at = ?2",
            params![concept_id, at],
        )?;
        Ok(())
    }

    /// When a concept was marked studied, if it was.
    pub fn studied_at(&self, concept_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT at FROM studied WHERE concept_id = ?1", [concept_id], |r| r.get(0)).optional()?)
    }

    pub fn start_batch(&self, id: &str, outcome_id: &str, provider: &str, now: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO item_batches (id, outcome_id, provider, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, outcome_id, provider, now],
        )?;
        Ok(())
    }

    pub fn finish_batch(&self, id: &str, counts: &BatchCounts, error: Option<&str>, now: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE item_batches SET asked = ?2, accepted = ?3, refused = ?4, prompt_tokens = ?5, completion_tokens = ?6,
             error = ?7, finished_at = ?8 WHERE id = ?1",
            params![
                id,
                counts.asked,
                counts.accepted,
                counts.refused,
                counts.prompt_tokens,
                counts.completion_tokens,
                error,
                now
            ],
        )?;
        Ok(())
    }

    /// How many writing runs an outcome has had (finished or not).
    /// Deletes a topic and, by cascade, everything in it. A suggestion that
    /// was started as this topic is set aside rather than left pointing
    /// at nothing.
    pub fn delete_topic(&self, topic_id: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("UPDATE suggestions SET status = 'dismissed', topic_id = NULL WHERE topic_id = ?1", [topic_id])?;
        tx.execute("DELETE FROM topics WHERE id = ?1", [topic_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Every batch's outcome, in the order the batches started.
    pub fn batch_order(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT outcome_id FROM item_batches ORDER BY rowid")?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
    }

    pub fn batch_count(&self, outcome_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT count(*) FROM item_batches WHERE outcome_id = ?1", [outcome_id], |r| r.get(0))?)
    }

    /// Accepted counts of an outcome's last `n` finished batches, newest
    /// first: two zeros in a row means back off.
    pub fn recent_batch_accepts(&self, outcome_id: &str, n: i64) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT accepted FROM item_batches WHERE outcome_id = ?1 AND finished_at IS NOT NULL AND error IS NULL
             ORDER BY finished_at DESC, id DESC LIMIT ?2",
        )?;
        Ok(stmt.query_map(params![outcome_id, n], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
    }
}

const SOURCE_COLS: &str =
    "id, concept_id, url, title, why, status, reason, words, checked_at, kind, content_json, automatic, added_by";

fn source(r: &rusqlite::Row) -> rusqlite::Result<SourceRow> {
    Ok(SourceRow {
        id: r.get(0)?,
        concept_id: r.get(1)?,
        url: r.get(2)?,
        title: r.get(3)?,
        why: r.get(4)?,
        status: r.get(5)?,
        reason: r.get(6)?,
        words: r.get(7)?,
        checked_at: r.get(8)?,
        kind: r.get(9)?,
        content_json: r.get(10)?,
        automatic: r.get::<_, i64>(11)? != 0,
        added_by: r.get(12)?,
    })
}

fn thread(r: &rusqlite::Row) -> rusqlite::Result<ThreadRow> {
    Ok(ThreadRow {
        id: r.get(0)?,
        topic_id: r.get(1)?,
        anchor_kind: r.get(2)?,
        anchor_id: r.get(3)?,
        title: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome_row(id: &str, topic: &str, ord: i64) -> OutcomeRow {
        OutcomeRow {
            id: id.into(),
            topic_id: topic.into(),
            ord,
            statement: format!("Outcome {ord}"),
            target_tier: "understand".into(),
            tier_reason: String::new(),
            concepts_state: "done".into(),
            bank_state: "waiting".into(),
            bank_error: None,
            retry_at: None,
        }
    }

    #[test]
    fn a_draft_is_shaped_then_confirmed() {
        let s = Store::open_in_memory().unwrap();
        s.create_draft_topic("t", "Containers", "explain to customers", "2026-10-25 09:00:00").unwrap();
        s.insert_outcome(&outcome_row("o1", "t", 1)).unwrap();
        s.insert_concept(&ConceptRow { id: "c1".into(), outcome_id: "o1".into(), ord: 1, statement: "x".into() })
            .unwrap();
        let shaped = vec![(
            outcome_row("o2", "t", 1),
            vec![ConceptRow { id: "c2".into(), outcome_id: "o2".into(), ord: 1, statement: "y".into() }],
        )];
        s.replace_draft("t", &shaped).unwrap();
        let outcomes = s.outcomes("t").unwrap();
        assert_eq!(outcomes.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), vec!["o2"]);
        assert_eq!(s.concepts("o2").unwrap()[0].statement, "y");
        assert!(s.concepts("o1").unwrap().is_empty(), "the old outcome's concepts went with it");
        s.confirm_topic("t", "2026-10-25 09:05:00").unwrap();
        let t = s.topic("t").unwrap().unwrap();
        assert_eq!((t.status.as_str(), t.confirmed_at.as_deref()), ("confirmed", Some("2026-10-25 09:05:00")));
    }

    #[test]
    fn items_count_toward_their_outcome_and_batches_record_accepts() {
        let s = Store::open_in_memory().unwrap();
        s.create_draft_topic("t", "T", "g", "2026-10-25 09:00:00").unwrap();
        s.insert_outcome(&outcome_row("o1", "t", 1)).unwrap();
        s.start_batch("b1", "o1", "local", "2026-10-25 09:01:00").unwrap();
        for (id, status) in [("i1", "active"), ("i2", "refused"), ("i3", "active")] {
            s.insert_item(&ItemRow {
                id: id.into(),
                topic_id: "t".into(),
                outcome_id: "o1".into(),
                batch_id: Some("b1".into()),
                kind: "recall".into(),
                tier: "understand".into(),
                concept_ids: vec!["c1".into()],
                stem: "?".into(),
                options_json: None,
                exemplar: None,
                source_ref: None,
                model_knowledge: true,
                status: status.into(),
                status_reason: None,
                created_at: "2026-10-25 09:01:00".into(),
            })
            .unwrap();
        }
        s.finish_batch(
            "b1",
            &BatchCounts { asked: 3, accepted: 2, refused: 1, ..Default::default() },
            None,
            "2026-10-25 09:02:00",
        )
        .unwrap();
        assert_eq!(s.active_counts("t").unwrap(), vec![("o1".to_string(), 2)]);
        assert_eq!(s.items("o1", Some("refused")).unwrap().len(), 1);
        assert_eq!(s.recent_batch_accepts("o1", 2).unwrap(), vec![2]);
        s.delete_topic("t").unwrap();
        assert!(s.items("o1", None).unwrap().is_empty(), "items go with their topic");
    }
}
