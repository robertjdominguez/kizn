-- Conversations with the tutor, kept with what they're about (a card, a
-- concept, a resource). The tutor's words never touch answers, states or
-- recalls: those come only from attempts.
CREATE TABLE threads (
  id           TEXT PRIMARY KEY,
  topic_id     TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  anchor_kind  TEXT NOT NULL,          -- card | concept | resource
  anchor_id    TEXT NOT NULL,          -- item id | concept id | source id
  title        TEXT NOT NULL,          -- the first question, for the list
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
) STRICT;
CREATE INDEX threads_anchor ON threads (anchor_kind, anchor_id, updated_at);

CREATE TABLE messages (
  id         TEXT PRIMARY KEY,
  thread_id  TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
  role       TEXT NOT NULL,            -- learner | tutor
  text       TEXT NOT NULL,
  at         TEXT NOT NULL
) STRICT;
CREATE INDEX messages_thread ON messages (thread_id, at);
