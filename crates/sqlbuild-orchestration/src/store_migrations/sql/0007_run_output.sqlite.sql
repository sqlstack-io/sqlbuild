-- A run's output lives in zstd objects in the output store, one per worker batch, and the control
-- store keeps only their index: each chunk, the line range of each node's stream in it, each
-- stream's totals and its error and warning lines, and what a size cap left out. Lines a node logs
-- with `ctx.log` are kept as the run's node logs.
CREATE TABLE sqb_run_output (
    run_id {uuid} PRIMARY KEY REFERENCES sqb_runs(run_id),
    next_chunk BIGINT NOT NULL,
    head_bytes BIGINT NOT NULL,
    tail_bytes BIGINT NOT NULL,
    stored_bytes BIGINT NOT NULL,
    omitted_bytes BIGINT NOT NULL,
    omitted_lines BIGINT NOT NULL,
    dropped_bytes BIGINT NOT NULL
);

CREATE TABLE sqb_run_output_chunks (
    run_id {uuid} NOT NULL REFERENCES sqb_run_output(run_id),
    chunk BIGINT NOT NULL,
    claim_id {uuid} NOT NULL,
    batch BIGINT NOT NULL,
    object_key TEXT NOT NULL,
    text_bytes BIGINT NOT NULL,
    stored_bytes BIGINT NOT NULL,
    part TEXT NOT NULL CHECK (part IN ('head', 'tail')),
    PRIMARY KEY (run_id, chunk),
    UNIQUE (run_id, claim_id, batch)
);

CREATE TABLE sqb_run_output_segments (
    run_id {uuid} NOT NULL,
    chunk BIGINT NOT NULL,
    segment BIGINT NOT NULL,
    node_key TEXT NOT NULL,
    stream TEXT NOT NULL CHECK (stream IN ('stdout', 'stderr')),
    first_line BIGINT NOT NULL,
    line_count BIGINT NOT NULL,
    text_offset BIGINT NOT NULL,
    text_bytes BIGINT NOT NULL,
    PRIMARY KEY (run_id, chunk, segment),
    FOREIGN KEY (run_id, chunk) REFERENCES sqb_run_output_chunks(run_id, chunk)
);

CREATE INDEX sqb_run_output_segments_lines ON sqb_run_output_segments(run_id, node_key, stream, first_line);

CREATE TABLE sqb_run_output_streams (
    run_id {uuid} NOT NULL REFERENCES sqb_run_output(run_id),
    node_key TEXT NOT NULL,
    stream TEXT NOT NULL CHECK (stream IN ('stdout', 'stderr')),
    line_count BIGINT NOT NULL,
    text_bytes BIGINT NOT NULL,
    error_lines {json} NOT NULL,
    warning_lines {json} NOT NULL,
    PRIMARY KEY (run_id, node_key, stream)
);

CREATE TABLE sqb_run_node_logs (
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    event_id {uuid} NOT NULL,
    node_key TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('info', 'warning', 'error')),
    message TEXT NOT NULL,
    logged_at {time} NOT NULL,
    PRIMARY KEY (run_id, event_id)
);
