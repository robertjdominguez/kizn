-- Reading in kizn: a resource keeps its text (a page's blocks, a video's
-- transcript cues) so the reader can show it and the tutor can discuss it.
-- kizn only offers what it can hold as text.
ALTER TABLE sources ADD COLUMN kind TEXT NOT NULL DEFAULT 'page';     -- page | video
ALTER TABLE sources ADD COLUMN content_json TEXT;                     -- [Block] | [Cue]
ALTER TABLE sources ADD COLUMN automatic INTEGER NOT NULL DEFAULT 0;  -- captions generated, not written
ALTER TABLE sources ADD COLUMN added_by TEXT NOT NULL DEFAULT 'kizn'; -- kizn | learner

-- One note per concept, the learner's own words; never counted.
CREATE TABLE notes (
  concept_id  TEXT PRIMARY KEY REFERENCES concepts(id) ON DELETE CASCADE,
  text        TEXT NOT NULL,
  updated_at  TEXT NOT NULL
) STRICT;
