-- A lecture per concept: written just in time (when the concept is opened,
-- or missed), every part citing a source kizn read and the checker verified.
-- body_json is kizn_core::lecture::Lecture, its cites source ids.
CREATE TABLE lectures (
  concept_id  TEXT PRIMARY KEY REFERENCES concepts(id) ON DELETE CASCADE,
  status      TEXT NOT NULL,       -- writing | ready | unbacked | failed
  body_json   TEXT,
  reason      TEXT,                -- why it's unbacked or failed
  written_at  TEXT NOT NULL
) STRICT;
