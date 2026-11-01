CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'partial', 'failed', 'cancelled', 'presumed_failed');
