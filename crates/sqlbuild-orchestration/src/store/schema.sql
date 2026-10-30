CREATE TABLE sqb_installation (
    singleton BIGINT PRIMARY KEY CHECK (singleton = 1),
    installation_id {uuid} NOT NULL,
    created_at {time} NOT NULL
);
CREATE TABLE sqb_locations (
    location_id {uuid} PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL,
    created_at {time} NOT NULL,
    archived_at {time},
    broken_deploy_policy TEXT NOT NULL DEFAULT 'fallback'
        CHECK (broken_deploy_policy IN ('fallback', 'queue'))
);
CREATE TABLE sqb_source_texts (
    text_hash TEXT PRIMARY KEY,
    body {binary} NOT NULL
);
CREATE TABLE sqb_node_definitions (
    node_hash TEXT PRIMARY KEY,
    body {binary} NOT NULL
);
CREATE TABLE sqb_documents (
    document_hash TEXT PRIMARY KEY,
    body {binary} NOT NULL
);
CREATE TABLE sqb_snapshots (
    snapshot_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    git_revision TEXT,
    git_dirty BOOLEAN NOT NULL,
    git_author TEXT,
    image_id TEXT,
    environment_hash TEXT NOT NULL,
    sqb_version TEXT NOT NULL,
    snapshot_schema_version BIGINT NOT NULL,
    manifest_digest TEXT NOT NULL,
    published_at {time} NOT NULL,
    UNIQUE (location_id, target, generation),
    UNIQUE (location_id, target, snapshot_id)
);
CREATE INDEX sqb_snapshots_location_time ON sqb_snapshots(location_id, target, published_at);
CREATE TABLE sqb_snapshot_nodes (
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    node_key TEXT NOT NULL,
    node_kind TEXT NOT NULL,
    node_hash TEXT NOT NULL REFERENCES sqb_node_definitions(node_hash),
    PRIMARY KEY (snapshot_id, node_key)
);
CREATE TABLE sqb_snapshot_sources (
    snapshot_id {uuid} NOT NULL,
    node_key TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('raw_sql', 'compiled_sql', 'python_source')),
    text_hash TEXT NOT NULL REFERENCES sqb_source_texts(text_hash),
    PRIMARY KEY (snapshot_id, node_key, kind),
    FOREIGN KEY (snapshot_id, node_key) REFERENCES sqb_snapshot_nodes(snapshot_id, node_key)
);
CREATE TABLE sqb_snapshot_documents (
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    kind TEXT NOT NULL CHECK (kind IN ('jobs', 'groups', 'triggers', 'roles', 'lineage', 'columns', 'providers')),
    document_hash TEXT NOT NULL REFERENCES sqb_documents(document_hash),
    PRIMARY KEY (snapshot_id, kind)
);
CREATE TABLE sqb_current_snapshots (
    location_id {uuid} NOT NULL,
    target TEXT NOT NULL,
    snapshot_id {uuid} NOT NULL,
    since {time} NOT NULL,
    PRIMARY KEY (location_id, target),
    FOREIGN KEY (location_id, target, snapshot_id)
        REFERENCES sqb_snapshots(location_id, target, snapshot_id)
);
CREATE TABLE sqb_snapshot_changes (
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    change_seq BIGINT NOT NULL,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('added', 'changed', 'removed')),
    subject_key TEXT NOT NULL,
    before_hash TEXT,
    after_hash TEXT,
    PRIMARY KEY (snapshot_id, change_seq)
);
CREATE TABLE sqb_scheduler_lease (
    name TEXT PRIMARY KEY CHECK (name = 'scheduler'),
    holder {uuid} NOT NULL,
    epoch BIGINT NOT NULL CHECK (epoch > 0),
    expires_at {time} NOT NULL
);
CREATE TABLE sqb_jobs (
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    job_key TEXT NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    kind TEXT NOT NULL CHECK (kind IN ('job', 'group')),
    definition_json {json} NOT NULL,
    overlap_policy TEXT NOT NULL CHECK (overlap_policy IN ('coalesce', 'queue', 'allow')),
    PRIMARY KEY (location_id, target, job_key)
);
CREATE TABLE sqb_job_nodes (
    location_id {uuid} NOT NULL,
    target TEXT NOT NULL,
    job_key TEXT NOT NULL,
    node_key TEXT NOT NULL,
    PRIMARY KEY (location_id, target, job_key, node_key),
    FOREIGN KEY (location_id, target, job_key) REFERENCES sqb_jobs(location_id, target, job_key)
);
CREATE TABLE sqb_triggers (
    trigger_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('schedule', 'sensor')),
    name TEXT NOT NULL,
    job_key TEXT NOT NULL,
    definition_json {json} NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    removed_in_snapshot_id {uuid} REFERENCES sqb_snapshots(snapshot_id),
    UNIQUE (location_id, target, kind, name)
);
CREATE TABLE sqb_trigger_state (
    trigger_id {uuid} PRIMARY KEY REFERENCES sqb_triggers(trigger_id),
    status TEXT NOT NULL CHECK (status IN ('running', 'paused')),
    status_override TEXT CHECK (status_override IN ('running', 'paused')),
    started_at {time} NOT NULL,
    cursor_json {json},
    last_tick_for {time},
    next_due_at {time},
    consecutive_failures BIGINT NOT NULL DEFAULT 0,
    updated_at {time} NOT NULL
);
CREATE INDEX sqb_trigger_state_due ON sqb_trigger_state(next_due_at) WHERE status = 'running';
CREATE TABLE sqb_trigger_state_events (
    event_id {uuid} PRIMARY KEY,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    occurred_at {time} NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    request_json {json} NOT NULL
);
CREATE INDEX sqb_trigger_state_events_time ON sqb_trigger_state_events(trigger_id, occurred_at, event_id);
CREATE TABLE sqb_ticks (
    tick_id {uuid} PRIMARY KEY,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    scheduled_for {time} NOT NULL,
    evaluated_at {time} NOT NULL,
    epoch BIGINT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('requested', 'skipped', 'covered', 'failed', 'deferred')),
    run_id {uuid},
    covered_by_run_id {uuid},
    reason TEXT,
    details_json {json} NOT NULL,
    UNIQUE (trigger_id, scheduled_for)
);
CREATE INDEX sqb_ticks_trigger_time ON sqb_ticks(trigger_id, scheduled_for);
CREATE TABLE sqb_runs (
    run_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    image_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('job', 'group', 'adhoc')),
    origin TEXT NOT NULL CHECK (origin IN ('manual', 'schedule', 'sensor', 'rerun', 'backfill')),
    job_key TEXT,
    parent_run_id {uuid} REFERENCES sqb_runs(run_id),
    root_run_id {uuid} NOT NULL,
    trigger_id {uuid} REFERENCES sqb_triggers(trigger_id),
    tick_id {uuid} REFERENCES sqb_ticks(tick_id),
    rerun_of_run_id {uuid} REFERENCES sqb_runs(run_id),
    requested_by TEXT NOT NULL,
    priority BIGINT NOT NULL DEFAULT 0,
    selection_json {json} NOT NULL,
    request_json {json} NOT NULL,
    write_set_digest TEXT NOT NULL,
    rewrite_history BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled', 'presumed_failed')),
    status_reason TEXT,
    queued_at {time} NOT NULL,
    launched_at {time},
    started_at {time},
    ended_at {time},
    cancel_requested_at {time},
    node_counts_json {json} NOT NULL,
    last_event_storage_id BIGINT NOT NULL
);
CREATE INDEX sqb_runs_location_time ON sqb_runs(location_id, target, queued_at);
CREATE INDEX sqb_runs_queue ON sqb_runs(priority DESC, queued_at) WHERE status = 'queued';
CREATE INDEX sqb_runs_active ON sqb_runs(location_id, target, job_key) WHERE status IN ('queued', 'launched', 'running', 'unresponsive');
CREATE TABLE sqb_run_nodes (
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    node_key TEXT NOT NULL,
    attempt BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'warn', 'failed', 'skipped', 'cancelled')),
    writes BOOLEAN NOT NULL,
    started_at {time},
    ended_at {time},
    rows_affected BIGINT,
    error_code TEXT,
    skipped_because_of TEXT,
    PRIMARY KEY (run_id, node_key, attempt)
);
CREATE INDEX sqb_run_nodes_node ON sqb_run_nodes(node_key, run_id);
CREATE TABLE sqb_write_claims (
    target TEXT NOT NULL,
    node_key TEXT NOT NULL,
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    claimed_at {time} NOT NULL,
    PRIMARY KEY (target, node_key)
);
CREATE INDEX sqb_write_claims_run ON sqb_write_claims(run_id);
CREATE TABLE sqb_concurrency_slots (
    group_key TEXT NOT NULL,
    slot BIGINT NOT NULL CHECK (slot > 0),
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    claimed_at {time} NOT NULL,
    PRIMARY KEY (group_key, slot),
    UNIQUE (group_key, run_id)
);
CREATE INDEX sqb_concurrency_slots_run ON sqb_concurrency_slots(run_id);
CREATE TABLE sqb_run_liveness (
    run_id {uuid} PRIMARY KEY REFERENCES sqb_runs(run_id),
    worker_instance {uuid} NOT NULL,
    container_id TEXT,
    claimed_at {time} NOT NULL,
    last_heartbeat_at {time} NOT NULL
);
CREATE TABLE sqb_principals (
    principal_id {uuid} PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('person', 'service_account')),
    created_at {time} NOT NULL,
    disabled_at {time}
);
CREATE TABLE sqb_access_tokens (
    token_id {uuid} PRIMARY KEY,
    secret_hash TEXT NOT NULL,
    principal_id {uuid} REFERENCES sqb_principals(principal_id),
    location_id {uuid} REFERENCES sqb_locations(location_id),
    run_id {uuid} REFERENCES sqb_runs(run_id),
    claim_id {uuid},
    scope_json {json} NOT NULL,
    created_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    revoked_at {time},
    CHECK (
        (principal_id IS NOT NULL AND location_id IS NULL AND run_id IS NULL AND claim_id IS NULL)
        OR (principal_id IS NULL AND location_id IS NOT NULL AND run_id IS NULL AND claim_id IS NULL)
        OR (principal_id IS NULL AND location_id IS NULL AND run_id IS NOT NULL AND claim_id IS NOT NULL)
    )
);
CREATE INDEX sqb_access_tokens_principal ON sqb_access_tokens(principal_id);
CREATE INDEX sqb_access_tokens_location ON sqb_access_tokens(location_id);
CREATE INDEX sqb_access_tokens_run ON sqb_access_tokens(run_id);
CREATE TABLE sqb_security_audit (
    event_id {uuid} PRIMARY KEY,
    occurred_at {time} NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    granting_role TEXT NOT NULL,
    request_id {uuid} NOT NULL,
    ip_address TEXT,
    scope_json {json} NOT NULL,
    before_json {json} NOT NULL,
    after_json {json} NOT NULL
);
CREATE INDEX sqb_security_audit_time ON sqb_security_audit(occurred_at, event_id);
