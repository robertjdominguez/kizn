-- Sources a concept can be studied from, suggested by the tutor and checked
-- by kizn before they're offered (only 'verified' ones are shown), and the
-- concepts the learner has marked studied. Studying never changes a
-- concept's state: states come only from answers.
CREATE TABLE sources (
  id          TEXT PRIMARY KEY,
  concept_id  TEXT NOT NULL REFERENCES concepts(id) ON DELETE CASCADE,
  url         TEXT NOT NULL,
  title       TEXT NOT NULL,
  why         TEXT,
  status      TEXT NOT NULL,          -- verified | rejected | failed
  reason      TEXT,                   -- "<label>: <detail>" when not verified
  words       INTEGER,
  checked_at  TEXT NOT NULL
) STRICT;
CREATE INDEX sources_concept ON sources (concept_id, status);

CREATE TABLE studied (
  concept_id  TEXT PRIMARY KEY REFERENCES concepts(id) ON DELETE CASCADE,
  at          TEXT NOT NULL
) STRICT;
