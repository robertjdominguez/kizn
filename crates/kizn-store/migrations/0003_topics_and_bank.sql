-- Topics get a lifecycle: a draft (being drafted or shaped) until the
-- learner confirms it. Outcomes and concepts hang off a topic; the bank
-- (items) and its writing runs (item_batches) hang off outcomes.
ALTER TABLE topics ADD COLUMN status TEXT NOT NULL DEFAULT 'draft';        -- draft | confirmed
ALTER TABLE topics ADD COLUMN draft_state TEXT NOT NULL DEFAULT 'drafting'; -- drafting | drafted | stopped | failed
ALTER TABLE topics ADD COLUMN draft_error TEXT;
ALTER TABLE topics ADD COLUMN confirmed_at TEXT;

CREATE TABLE outcomes (
  id              TEXT PRIMARY KEY,
  topic_id        TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  ord             INTEGER NOT NULL,
  statement       TEXT NOT NULL,
  target_tier     TEXT NOT NULL,                      -- kizn_core::Tier
  tier_reason     TEXT NOT NULL DEFAULT '',
  concepts_state  TEXT NOT NULL DEFAULT 'pending',    -- pending | drafting | done
  bank_state      TEXT NOT NULL DEFAULT 'waiting',    -- waiting | writing | ready | failed
  bank_error      TEXT,
  retry_at        TEXT
) STRICT;
CREATE INDEX outcomes_topic ON outcomes (topic_id, ord);

CREATE TABLE concepts (
  id          TEXT PRIMARY KEY,
  outcome_id  TEXT NOT NULL REFERENCES outcomes(id) ON DELETE CASCADE,
  ord         INTEGER NOT NULL,
  statement   TEXT NOT NULL
) STRICT;
CREATE INDEX concepts_outcome ON concepts (outcome_id, ord);

CREATE TABLE item_batches (
  id                 TEXT PRIMARY KEY,
  outcome_id         TEXT NOT NULL REFERENCES outcomes(id) ON DELETE CASCADE,
  provider           TEXT NOT NULL,
  asked              INTEGER NOT NULL DEFAULT 0,
  accepted           INTEGER NOT NULL DEFAULT 0,
  refused            INTEGER NOT NULL DEFAULT 0,
  prompt_tokens      INTEGER NOT NULL DEFAULT 0,
  completion_tokens  INTEGER NOT NULL DEFAULT 0,
  started_at         TEXT NOT NULL,
  finished_at        TEXT,
  error              TEXT
) STRICT;

CREATE TABLE items (
  id               TEXT PRIMARY KEY,
  topic_id         TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  outcome_id       TEXT NOT NULL REFERENCES outcomes(id) ON DELETE CASCADE,
  batch_id         TEXT REFERENCES item_batches(id) ON DELETE SET NULL,
  kind             TEXT NOT NULL,                     -- recall | apply | target | explain
  tier             TEXT NOT NULL,
  concept_ids_json TEXT NOT NULL,
  stem             TEXT NOT NULL,
  options_json     TEXT,                              -- kizn_core::bank::ItemOption[]
  exemplar         TEXT,
  source_ref       TEXT,
  model_knowledge  INTEGER NOT NULL DEFAULT 1,
  status           TEXT NOT NULL,                     -- active | refused | retired
  status_reason    TEXT,                              -- "<label>: <detail>" when refused
  created_at       TEXT NOT NULL
) STRICT;
CREATE INDEX items_outcome ON items (outcome_id, status, kind);
