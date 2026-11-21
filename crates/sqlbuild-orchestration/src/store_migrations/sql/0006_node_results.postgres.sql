-- A run's nodes say what each one did: the rows a rebuilt table holds, rows written so far while
-- it runs, and why a node that ran changed nothing. A node the run never reached is `not_run`,
-- apart from a node something upstream skipped.
ALTER TABLE sqb_run_nodes DROP CONSTRAINT IF EXISTS sqb_run_nodes_status_check;

ALTER TABLE sqb_run_nodes ADD CONSTRAINT sqb_run_nodes_status_check CHECK (status IN ('pending', 'running', 'success', 'warn', 'failed', 'skipped', 'cancelled', 'not_run'));

ALTER TABLE sqb_run_nodes ADD COLUMN IF NOT EXISTS table_rows BIGINT;

ALTER TABLE sqb_run_nodes ADD COLUMN IF NOT EXISTS rows_so_far BIGINT;

ALTER TABLE sqb_run_nodes ADD COLUMN IF NOT EXISTS no_change_reason TEXT;
