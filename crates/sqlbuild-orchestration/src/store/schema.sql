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
    archived_at {time}
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
    PRIMARY KEY (location_id, target, job_key)
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
    updated_at {time} NOT NULL
);
CREATE INDEX sqb_trigger_state_due ON sqb_trigger_state(next_due_at) WHERE status = 'running';
CREATE TABLE sqb_ticks (
    tick_id {uuid} PRIMARY KEY,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    scheduled_for {time} NOT NULL,
    evaluated_at {time} NOT NULL,
    epoch BIGINT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('requested', 'covered')),
    run_id {uuid},
    covered_by_run_id {uuid},
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
    authored_json {json},
    write_set_digest TEXT NOT NULL,
    rewrite_history BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled')),
    status_reason TEXT,
    queued_at {time} NOT NULL,
    launched_at {time},
    started_at {time},
    ended_at {time},
    cancel_requested_at {time},
    node_counts_json {json} NOT NULL,
    last_event_storage_id BIGINT NOT NULL,
    worker_instance {uuid},
    container_id TEXT,
    claimed_at {time},
    last_heartbeat_at {time}
);
CREATE INDEX sqb_runs_location_time ON sqb_runs(location_id, target, queued_at);
CREATE INDEX sqb_runs_queue ON sqb_runs(priority DESC, queued_at) WHERE status = 'queued';
CREATE INDEX sqb_runs_active ON sqb_runs(location_id, target, job_key) WHERE status IN ('queued', 'launched', 'running', 'unresponsive');
CREATE TABLE sqb_run_nodes (
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    node_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'warn', 'failed', 'skipped', 'cancelled')),
    writes BOOLEAN NOT NULL,
    started_at {time},
    ended_at {time},
    rows_affected BIGINT,
    error_code TEXT,
    skipped_because_of TEXT,
    kind TEXT NOT NULL DEFAULT 'node' CHECK (kind IN ('node', 'check')),
    finished_storage_id BIGINT,
    PRIMARY KEY (run_id, node_key)
);
CREATE INDEX sqb_run_nodes_node ON sqb_run_nodes(node_key, run_id);
CREATE INDEX sqb_run_nodes_finished ON sqb_run_nodes(finished_storage_id)
WHERE finished_storage_id IS NOT NULL;
CREATE INDEX sqb_run_nodes_decisive ON sqb_run_nodes(node_key, finished_storage_id)
WHERE finished_storage_id IS NOT NULL AND status IN ('success', 'failed');
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
CREATE TABLE sqb_principals (
    principal_id {uuid} PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at {time} NOT NULL,
    disabled_at {time}
);
CREATE TABLE sqb_access_tokens (
    token_id {uuid} PRIMARY KEY,
    secret_hash TEXT NOT NULL,
    location_id {uuid} REFERENCES sqb_locations(location_id),
    run_id {uuid} REFERENCES sqb_runs(run_id),
    claim_id {uuid},
    scope_json {json} NOT NULL,
    created_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    revoked_at {time},
    CHECK (
        (location_id IS NOT NULL AND run_id IS NULL AND claim_id IS NULL)
        OR (location_id IS NULL AND run_id IS NOT NULL AND claim_id IS NOT NULL)
    )
);
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
    after_json {json} NOT NULL,
    storage_id BIGINT
);
CREATE INDEX sqb_security_audit_time ON sqb_security_audit(occurred_at, event_id);
CREATE INDEX sqb_security_audit_trigger ON sqb_security_audit((scope_json ->> 'trigger_id'), occurred_at, event_id)
WHERE action = 'trigger.changed';
CREATE TABLE sqb_location_instances (
    instance_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    connected_at {time} NOT NULL,
    last_seen_at {time} NOT NULL,
    health_json {json} NOT NULL
);
CREATE INDEX sqb_location_instances_location_seen ON sqb_location_instances(location_id, last_seen_at);
CREATE TABLE sqb_location_work (
    request_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    generation BIGINT NOT NULL,
    request_json {json} NOT NULL,
    requested_by TEXT NOT NULL,
    requested_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    claim_id {uuid},
    session_id {uuid} REFERENCES sqb_location_instances(instance_id),
    claimed_until {time},
    result_json {json},
    completed_at {time}
);
CREATE INDEX sqb_location_work_pending ON sqb_location_work(location_id, target, requested_at);
CREATE TABLE sqb_platform_sink_deliveries (
    sink_id TEXT NOT NULL,
    storage_id BIGINT NOT NULL,
    event_id TEXT NOT NULL,
    delivered_at {time} NOT NULL,
    PRIMARY KEY (sink_id, storage_id)
);
CREATE TABLE sqb_sensor_evaluation_events (
    event_id {uuid} PRIMARY KEY,
    evaluation_id {uuid} NOT NULL,
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    phase TEXT NOT NULL CHECK (phase IN ('requested', 'completed', 'failed', 'timed_out', 'abandoned')),
    occurred_at {time} NOT NULL,
    fact_json {json} NOT NULL,
    UNIQUE (evaluation_id, phase)
);
CREATE INDEX sqb_sensor_evaluation_history ON sqb_sensor_evaluation_events(trigger_id, phase, occurred_at, evaluation_id);
CREATE INDEX sqb_sensor_evaluation_expiry ON sqb_sensor_evaluation_events(occurred_at);
CREATE TABLE sqb_run_requests (
    trigger_id {uuid} NOT NULL REFERENCES sqb_triggers(trigger_id),
    run_key TEXT NOT NULL,
    evaluation_id {uuid} NOT NULL,
    run_id {uuid} NOT NULL REFERENCES sqb_runs(run_id),
    PRIMARY KEY (trigger_id, run_key)
);
CREATE INDEX sqb_runs_terminal_cursor ON sqb_runs(location_id, target, job_key, last_event_storage_id)
WHERE status IN ('success', 'warn', 'partial', 'failed', 'cancelled');
CREATE INDEX sqb_runs_parent ON sqb_runs(parent_run_id) WHERE parent_run_id IS NOT NULL;
CREATE TABLE sqb_warehouse_freshness (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT,
    scan_id {uuid}
);
CREATE INDEX sqb_warehouse_freshness_scope ON sqb_warehouse_freshness (location_id, target, observed_at, event_id);
CREATE TABLE sqb_warehouse_fingerprints (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    definition_text_hash TEXT NOT NULL REFERENCES sqb_source_texts(text_hash),
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT,
    node_type TEXT,
    node_name TEXT,
    target_database TEXT,
    target_schema TEXT,
    target_name TEXT,
    scan_id {uuid}
);
CREATE INDEX sqb_warehouse_fingerprints_scope ON sqb_warehouse_fingerprints (location_id, target, observed_at, event_id);
CREATE TABLE sqb_development_locations (
    project_path TEXT NOT NULL,
    target TEXT NOT NULL,
    location_id {uuid} NOT NULL UNIQUE REFERENCES sqb_locations(location_id),
    PRIMARY KEY (project_path, target)
);
CREATE TABLE sqb_password_credentials (
    principal_id {uuid} PRIMARY KEY REFERENCES sqb_principals(principal_id),
    credential_id {uuid} NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    changed_at {time} NOT NULL
);
CREATE TABLE sqb_sessions (
    session_id {uuid} PRIMARY KEY,
    principal_id {uuid} NOT NULL REFERENCES sqb_principals(principal_id),
    credential_id {uuid},
    secret_hash TEXT NOT NULL,
    csrf_hash TEXT NOT NULL,
    created_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    revoked_at {time}
);
CREATE INDEX sqb_sessions_person ON sqb_sessions(principal_id);
CREATE TABLE sqb_password_login_attempts (
    attempt_id {uuid} PRIMARY KEY,
    identity_key TEXT NOT NULL,
    peer_address TEXT NOT NULL,
    attempted_at {time} NOT NULL
);
CREATE INDEX sqb_password_login_user_time ON sqb_password_login_attempts(identity_key, attempted_at);
CREATE INDEX sqb_password_login_peer_time ON sqb_password_login_attempts(peer_address, attempted_at);
CREATE INDEX sqb_password_login_time ON sqb_password_login_attempts(attempted_at);
CREATE TABLE sqb_oidc_login_flows (
    flow_id {uuid} PRIMARY KEY,
    configuration_id TEXT NOT NULL,
    state_hash TEXT NOT NULL UNIQUE,
    browser_hash TEXT NOT NULL,
    nonce TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    expires_at {time} NOT NULL
);
CREATE INDEX sqb_oidc_login_flows_expiry ON sqb_oidc_login_flows(expires_at);
CREATE TABLE sqb_role_assignments (
    assignment_id {uuid} PRIMARY KEY,
    principal_id {uuid} NOT NULL REFERENCES sqb_principals(principal_id),
    role_key TEXT NOT NULL,
    location_id {uuid} REFERENCES sqb_locations(location_id),
    target TEXT,
    active_key TEXT NOT NULL UNIQUE
);
CREATE INDEX sqb_role_assignments_principal ON sqb_role_assignments(principal_id);
CREATE INDEX sqb_fingerprints_namespace ON sqb_warehouse_fingerprints (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE INDEX sqb_freshness_namespace ON sqb_warehouse_freshness (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE UNIQUE INDEX sqb_security_audit_storage_id ON sqb_security_audit(storage_id);
CREATE TABLE sqb_warehouse_audit_results (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    audit_name TEXT NOT NULL,
    outcome TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    scan_id {uuid}
);
CREATE INDEX sqb_audit_results_namespace ON sqb_warehouse_audit_results (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE TABLE sqb_warehouse_janitor_events (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    event_type TEXT NOT NULL,
    relation_database TEXT,
    relation_schema TEXT NOT NULL,
    archive_name TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    scan_id {uuid}
);
CREATE INDEX sqb_janitor_events_namespace ON sqb_warehouse_janitor_events (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE TABLE sqb_warehouse_migrations (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    target_name TEXT,
    destination_model TEXT NOT NULL,
    decision TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    scan_id {uuid}
);
CREATE INDEX sqb_migrations_namespace ON sqb_warehouse_migrations (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE TABLE sqb_warehouse_old_name_views (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    target_name TEXT,
    migration_event_id TEXT NOT NULL,
    destination_model TEXT NOT NULL,
    event_type TEXT NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    scan_id {uuid}
);
CREATE INDEX sqb_old_name_views_namespace ON sqb_warehouse_old_name_views (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE TABLE sqb_warehouse_microbatches (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at {time} NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    model_name TEXT NOT NULL,
    target_name TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    physical_generation_id TEXT NOT NULL,
    record_type TEXT NOT NULL,
    origin_run_id TEXT NOT NULL,
    execution_run_id TEXT NOT NULL,
    replay_requirement_id TEXT,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    scan_id {uuid}
);
CREATE INDEX sqb_microbatches_namespace ON sqb_warehouse_microbatches (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE INDEX recall_warehouse_sources ON recall_events (storage_id)
    WHERE producer='warehouse' AND event_type='warehouse_source_freshness'
      AND invocation_id LIKE 'warehouse/source-freshness/%';
CREATE INDEX sqb_freshness_source_latest ON sqb_warehouse_freshness (
    location_id, target, (record_json ->> 'source_name'),
    (record_json ->> 'target_database'), (record_json ->> 'target_schema'),
    (record_json ->> 'target_name'), observed_at DESC, event_id DESC
);
CREATE TABLE sqb_security_audit_deliveries (
    sink_id TEXT NOT NULL,
    storage_id BIGINT NOT NULL,
    event_id TEXT NOT NULL,
    delivered_at {time} NOT NULL,
    PRIMARY KEY (sink_id, storage_id)
);
CREATE TABLE sqb_warehouse_catalog (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    observed_at {time} NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL,
    scan_id {uuid}
);
CREATE INDEX sqb_warehouse_catalog_namespace ON sqb_warehouse_catalog
    (location_id, target, state_schema, state_database, observed_at, event_id);
CREATE INDEX sqb_fingerprint_node_latest ON sqb_warehouse_fingerprints
    (location_id,target,node_type,node_name,target_database,target_schema,target_name,observed_at DESC,event_id DESC);
CREATE TABLE sqb_warehouse_statistics (
    event_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    state_database TEXT,
    state_schema TEXT NOT NULL,
    relation_name TEXT NOT NULL,
    observed_at {time} NOT NULL,
    record_json {json} NOT NULL,
    received_at {time} NOT NULL
);
CREATE INDEX sqb_statistics_relation_time ON sqb_warehouse_statistics
    (location_id,target,state_schema,state_database,relation_name,observed_at,event_id);
CREATE INDEX sqb_statistics_expiration ON sqb_warehouse_statistics (observed_at,event_id);
CREATE INDEX sqb_catalog_statistics_scan ON sqb_warehouse_catalog
    (location_id,target,state_schema,state_database,event_id);
CREATE TABLE sqb_cli_login_flows (
    flow_id {uuid} PRIMARY KEY,
    client_hash TEXT NOT NULL,
    browser_hash TEXT NOT NULL,
    expires_at {time} NOT NULL,
    principal_id {uuid},
    session_id {uuid}
);
CREATE INDEX sqb_cli_login_expiry ON sqb_cli_login_flows(expires_at);
CREATE INDEX IF NOT EXISTS sqb_runs_tick ON sqb_runs(tick_id);
CREATE INDEX IF NOT EXISTS sqb_ticks_expiry ON sqb_ticks(scheduled_for) WHERE outcome = 'covered';
CREATE INDEX IF NOT EXISTS sqb_runs_snapshot ON sqb_runs(snapshot_id);
CREATE INDEX IF NOT EXISTS sqb_snapshot_nodes_hash ON sqb_snapshot_nodes(node_hash);
CREATE INDEX IF NOT EXISTS sqb_snapshot_sources_hash ON sqb_snapshot_sources(text_hash);
CREATE INDEX IF NOT EXISTS sqb_snapshot_documents_hash ON sqb_snapshot_documents(document_hash);
CREATE TABLE sqb_effective_roles (
    location_id {uuid} NOT NULL,
    target TEXT NOT NULL,
    role_key TEXT NOT NULL,
    resolved_grants_json TEXT NOT NULL,
    PRIMARY KEY (location_id, target, role_key)
);
