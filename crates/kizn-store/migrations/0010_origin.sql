-- Who set a topic up: the learner, or kizn (a shaky foundation found by
-- checking the ground, prepared in the background).
ALTER TABLE topics ADD COLUMN origin TEXT NOT NULL DEFAULT 'learner';
