-- The map: links between topics, each from an upper topic down to a lower
-- one. "rests_on": the lower topic is a foundation of the upper. "part_of":
-- the lower topic is a piece of the upper. Any depth, fanning out both ways;
-- loops are refused in code before a link is stored.
CREATE TABLE links (
  id          TEXT PRIMARY KEY,
  upper_id    TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  lower_id    TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,          -- rests_on | part_of
  created_at  TEXT NOT NULL,
  UNIQUE (upper_id, lower_id)
) STRICT;

-- A foundation proposed for a topic that isn't a link yet: from the draft,
-- from checking the ground, or from misses. Started (a new topic, linked),
-- linked to a topic the learner has, or dismissed.
CREATE TABLE suggestions (
  id          TEXT PRIMARY KEY,
  for_topic   TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  title       TEXT NOT NULL,
  why         TEXT,
  source      TEXT NOT NULL,          -- draft | check | flag
  status      TEXT NOT NULL,          -- open | kept | started | linked | dismissed
  topic_id    TEXT REFERENCES topics(id) ON DELETE SET NULL,
  created_at  TEXT NOT NULL
) STRICT;
CREATE INDEX suggestions_topic ON suggestions (for_topic, status);
