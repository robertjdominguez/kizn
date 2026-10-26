-- Answers: the only record practice keeps. Concept states, recall dates and
-- gates are worked out from these (and the items they answer), never stored.
-- Append-only; a dispute (Phase 10) marks one void rather than deleting it.
CREATE TABLE attempts (
  id          TEXT PRIMARY KEY,
  topic_id    TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  outcome_id  TEXT NOT NULL REFERENCES outcomes(id) ON DELETE CASCADE,
  item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  order_json  TEXT NOT NULL,          -- the stored options' indices, in the order shown
  chosen      INTEGER NOT NULL,       -- index into the stored options
  correct     INTEGER NOT NULL,
  at          TEXT NOT NULL,          -- UTC 'YYYY-MM-DD HH:MM:SS', from the Clock
  disputed_at TEXT
) STRICT;
CREATE INDEX attempts_topic ON attempts (topic_id, at);
CREATE INDEX attempts_item ON attempts (item_id, at);
