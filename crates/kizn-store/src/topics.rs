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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchCounts {
    pub asked: i64,
    pub accepted: i64,
    pub refused: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

const TOPIC_COLS: &str = "id, title, goal_md, status, draft_state, draft_error, created_at, confirmed_at";
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
        self.conn.lock().unwrap().execute(
            "INSERT INTO topics (id, title, goal_md, created_at, status, draft_state) VALUES (?1, ?2, ?3, ?4, 'draft', 'drafting')",
            params![id, title, goal, now],
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

    pub fn delete_topic(&self, topic_id: &str) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM topics WHERE id = ?1", [topic_id])?;
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
