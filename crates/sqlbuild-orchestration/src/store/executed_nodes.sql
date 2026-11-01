CREATE INDEX sqb_runs_parent ON sqb_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;
