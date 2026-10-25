-- Topics: the unit of the map. Outcomes, concepts and the bank arrive with
-- the phases whose scenarios need them.
CREATE TABLE topics (
  id         TEXT PRIMARY KEY,   -- ULID
  title      TEXT NOT NULL,
  goal_md    TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL       -- UTC 'YYYY-MM-DD HH:MM:SS', from the Clock
) STRICT;
