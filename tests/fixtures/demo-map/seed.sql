-- Loaded after topic-with-bank by `just demo fresh`: topics around "Container
-- fundamentals" (01SEEDTOPIC00000000000000T), linked in four layers, with a
-- fan-in, a part-of and two suggestions for later. No outcomes: map only.
INSERT INTO topics (id, title, goal_md, created_at, status, draft_state, confirmed_at) VALUES
  ('01DEMOMAP00000000000000001', 'SPCS', 'run my services on Snowpark Container Services', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00'),
  ('01DEMOMAP00000000000000002', 'Advising customers on SPCS', 'advise customers on moving workloads to SPCS', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00'),
  ('01DEMOMAP00000000000000003', 'SPCS networking', 'connect services inside SPCS', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00'),
  ('01DEMOMAP00000000000000004', 'Linux processes', 'understand processes, signals and isolation', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00'),
  ('01DEMOMAP00000000000000005', 'Networking basics', 'ports, addresses and DNS', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00'),
  ('01DEMOMAP00000000000000006', 'Operating systems basics', 'what a kernel does', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00'),
  ('01DEMOMAP00000000000000007', 'Snowflake SQL', 'query and load data in Snowflake', '2026-10-25 09:00:00', 'confirmed', 'drafted', '2026-10-25 09:00:00');
INSERT INTO links (id, upper_id, lower_id, kind, created_at) VALUES
  ('01DEMOLINK0000000000000001', '01DEMOMAP00000000000000002', '01DEMOMAP00000000000000001', 'rests_on', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000002', '01DEMOMAP00000000000000002', '01DEMOMAP00000000000000007', 'rests_on', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000003', '01DEMOMAP00000000000000001', '01SEEDTOPIC00000000000000T', 'rests_on', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000004', '01DEMOMAP00000000000000001', '01DEMOMAP00000000000000003', 'part_of', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000005', '01SEEDTOPIC00000000000000T', '01DEMOMAP00000000000000004', 'rests_on', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000006', '01DEMOMAP00000000000000003', '01DEMOMAP00000000000000005', 'rests_on', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000007', '01DEMOMAP00000000000000004', '01DEMOMAP00000000000000006', 'rests_on', '2026-10-25 09:00:00'),
  ('01DEMOLINK0000000000000008', '01DEMOMAP00000000000000005', '01DEMOMAP00000000000000006', 'rests_on', '2026-10-25 09:00:00');
INSERT INTO suggestions (id, for_topic, title, why, source, status, topic_id, created_at) VALUES
  ('01DEMOSUGG0000000000000001', '01SEEDTOPIC00000000000000T', 'Cgroups and namespaces', 'containers are isolated with cgroups and namespaces', 'draft', 'later', NULL, '2026-10-25 09:00:00'),
  ('01DEMOSUGG0000000000000002', '01DEMOMAP00000000000000001', 'Kubernetes basics', 'SPCS schedules services much like Kubernetes', 'flag', 'later', NULL, '2026-10-25 09:00:00');
