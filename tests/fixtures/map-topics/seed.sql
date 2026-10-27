-- Seeded by `just seed map-topics`: topics to link on the map (no outcomes yet).
DELETE FROM topics;
INSERT INTO topics (id, title, goal_md, created_at, status, draft_state, confirmed_at) VALUES ('01MAPTOPIC0000000000000001', 'SPCS', 'run my services on Snowpark Container Services', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00');
INSERT INTO topics (id, title, goal_md, created_at, status, draft_state, confirmed_at) VALUES ('01MAPTOPIC0000000000000002', 'Container fundamentals', 'explain how containers work', '2026-10-25 09:01:00', 'confirmed', 'drafted', '2026-10-25 09:01:00');
INSERT INTO topics (id, title, goal_md, created_at, status, draft_state, confirmed_at) VALUES ('01MAPTOPIC0000000000000003', 'Linux processes', 'understand processes and signals', '2026-10-25 09:02:00', 'confirmed', 'drafted', '2026-10-25 09:02:00');
INSERT INTO topics (id, title, goal_md, created_at, status, draft_state, confirmed_at) VALUES ('01MAPTOPIC0000000000000004', 'SPCS networking', 'connect services inside SPCS', '2026-10-25 09:03:00', 'confirmed', 'drafted', '2026-10-25 09:03:00');
