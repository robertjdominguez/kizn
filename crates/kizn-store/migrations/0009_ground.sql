-- Checking the ground: a few questions on each foundation kept for a new
-- topic, and the learner's answers. Diagnostic only: they never touch the
-- topic's concepts, states or recalls.
CREATE TABLE ground_items (
  id             TEXT PRIMARY KEY,
  suggestion_id  TEXT NOT NULL REFERENCES suggestions(id) ON DELETE CASCADE,
  stem           TEXT NOT NULL,
  options_json   TEXT NOT NULL,          -- kizn_core::bank::ItemOption[]
  created_at     TEXT NOT NULL
) STRICT;

CREATE TABLE ground_answers (
  item_id  TEXT PRIMARY KEY REFERENCES ground_items(id) ON DELETE CASCADE,
  chosen   INTEGER NOT NULL,             -- index into the stored options
  correct  INTEGER NOT NULL,
  at       TEXT NOT NULL
) STRICT;
