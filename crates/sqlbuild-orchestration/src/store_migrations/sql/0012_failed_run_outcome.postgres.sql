-- 'partial' is no longer a run outcome: a run that built some nodes and not others ended `failed`.
-- Stored runs, their terminal facts, run_failed events and sensor ticks say `failed` instead, and
-- the data step makes sensors watching `partial` runs watch `failed` ones.
UPDATE sqb_runs SET status = 'failed' WHERE status = 'partial';

ALTER TABLE sqb_runs DROP CONSTRAINT sqb_runs_status_check;

ALTER TABLE sqb_runs ADD CONSTRAINT sqb_runs_status_check CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'failed', 'cancelled', 'skipped'));

DROP INDEX sqb_runs_terminal_cursor;

CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'failed', 'cancelled', 'skipped');

UPDATE recall_events SET payload = jsonb_set(payload, '{change,change,outcome}', '"failed"')
WHERE event_type = 'run_terminated' AND payload #>> '{change,change,outcome}' = 'partial';

UPDATE recall_events SET payload = jsonb_set(payload, '{platform,outcome}', '"failed"')
WHERE event_type = 'platform_run_failed' AND payload #>> '{platform,outcome}' = 'partial';

UPDATE sqb_sensor_evaluation_events SET fact_json = jsonb_set(fact_json, '{request,event,status}', '"failed"')
WHERE fact_json #>> '{request,event,status}' = 'partial';

UPDATE sqb_sensor_evaluation_events
SET fact_json = jsonb_set(fact_json, '{request,event,details,outcome}', '"failed"')
WHERE fact_json #>> '{request,event,details,outcome}' = 'partial';
