-- A run's nodes say what each one did: the rows a rebuilt table holds, rows written so far while
-- it runs, and why a node that ran changed nothing. A node the run never reached is `not_run`,
-- apart from a node something upstream skipped.
CREATE TABLE sqb_run_nodes_v2 (
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    node_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'warn', 'failed', 'skipped', 'cancelled', 'not_run')),
    writes BOOLEAN NOT NULL,
    started_at {time},
    ended_at {time},
    rows_affected BIGINT,
    error_code TEXT,
    skipped_because_of TEXT,
    kind TEXT NOT NULL DEFAULT 'node' CHECK (kind IN ('node', 'check')),
    finished_storage_id BIGINT,
    table_rows BIGINT,
    rows_so_far BIGINT,
    no_change_reason TEXT,
    PRIMARY KEY (run_id, node_key)
);

INSERT INTO sqb_run_nodes_v2 (run_id, node_key, status, writes, started_at, ended_at, rows_affected, error_code, skipped_because_of, kind, finished_storage_id) SELECT run_id, node_key, status, writes, started_at, ended_at, rows_affected, error_code, skipped_because_of, kind, finished_storage_id FROM sqb_run_nodes;

DROP TABLE sqb_run_nodes;

ALTER TABLE sqb_run_nodes_v2 RENAME TO sqb_run_nodes;

CREATE INDEX sqb_run_nodes_node ON sqb_run_nodes(node_key, run_id);

CREATE INDEX sqb_run_nodes_finished ON sqb_run_nodes(finished_storage_id) WHERE finished_storage_id IS NOT NULL;

CREATE INDEX sqb_run_nodes_decisive ON sqb_run_nodes(node_key, finished_storage_id) WHERE finished_storage_id IS NOT NULL AND status IN ('success', 'failed');
