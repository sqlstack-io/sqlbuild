INSERT INTO sqb_locations (location_id, name, created_by, created_at) VALUES ('018f0000-0000-7000-8000-000000000001', 'orders', 'admin', '2026-11-01T09:00:00Z');

INSERT INTO sqb_snapshots (snapshot_id, location_id, target, generation, git_dirty, environment_hash, sqb_version, manifest_digest, published_at) VALUES ('018f0000-0000-7000-8000-000000000002', '018f0000-0000-7000-8000-000000000001', 'prod', 1, FALSE, 'env-1', '0.241.0', 'manifest-1', '2026-11-01T09:05:00Z');

INSERT INTO sqb_principals (principal_id, name, created_at) VALUES ('018f0000-0000-7000-8000-000000000003', 'ana', '2026-11-01T09:01:00Z');

INSERT INTO sqb_role_assignments (assignment_id, principal_id, role_key, location_id, target, active_key) VALUES ('018f0000-0000-7000-8000-000000000004', '018f0000-0000-7000-8000-000000000003', 'operator', '018f0000-0000-7000-8000-000000000001', 'prod', 'ana:operator:orders:prod');

INSERT INTO sqb_access_tokens (token_id, secret_hash, location_id, scope_json, created_at, expires_at) VALUES ('018f0000-0000-7000-8000-000000000005', 'hash-1', '018f0000-0000-7000-8000-000000000001', '{"kind": "location"}', '2026-11-01T09:02:00Z', '2027-11-01T09:02:00Z');

INSERT INTO sqb_triggers (trigger_id, location_id, target, kind, name, job_key, definition_json, snapshot_id) VALUES ('018f0000-0000-7000-8000-000000000006', '018f0000-0000-7000-8000-000000000001', 'prod', 'schedule', 'nightly', 'daily_orders', '{"cron": "0 2 * * *"}', '018f0000-0000-7000-8000-000000000002');

INSERT INTO sqb_trigger_state (trigger_id, status, status_override, started_at, next_due_at) VALUES ('018f0000-0000-7000-8000-000000000006', 'paused', 'paused', '2026-11-01T09:06:00Z', '2026-11-02T02:00:00Z');

INSERT INTO sqb_runs (run_id, location_id, target, snapshot_id, kind, origin, job_key, root_run_id, requested_by, selection_json, request_json, write_set_digest, status, queued_at, ended_at, node_counts_json, last_event_storage_id) VALUES ('018f0000-0000-7000-8000-000000000007', '018f0000-0000-7000-8000-000000000001', 'prod', '018f0000-0000-7000-8000-000000000002', 'job', 'manual', 'daily_orders', '018f0000-0000-7000-8000-000000000007', 'principal:018f0000-0000-7000-8000-000000000003', '[]', '{}', 'digest-1', 'success', '2026-11-01T10:00:00Z', '2026-11-01T10:05:00Z', '{"success": 3}', 12);

INSERT INTO sqb_run_nodes (run_id, node_key, status, writes, started_at, ended_at, rows_affected, finished_storage_id) VALUES ('018f0000-0000-7000-8000-000000000007', 'model.orders', 'success', TRUE, '2026-11-01T10:01:00Z', '2026-11-01T10:02:00Z', 3, 11);
