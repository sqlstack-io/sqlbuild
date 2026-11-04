CREATE TABLE IF NOT EXISTS sqb_role_definition_versions (
    location_id {uuid} NOT NULL REFERENCES sqb_locations(location_id),
    target TEXT NOT NULL,
    role_key TEXT NOT NULL,
    definition_digest TEXT NOT NULL,
    definition_json TEXT NOT NULL,
    first_seen_snapshot_id {uuid} NOT NULL,
    first_seen_at {time} NOT NULL,
    PRIMARY KEY (location_id, target, role_key, definition_digest)
);
CREATE TABLE IF NOT EXISTS sqb_role_activation_requests (
    request_id {uuid} PRIMARY KEY,
    location_id {uuid} NOT NULL,
    target TEXT NOT NULL,
    role_key TEXT NOT NULL,
    definition_digest TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    requested_at {time} NOT NULL,
    reason TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sqb_role_approvals (
    approval_id {uuid} PRIMARY KEY,
    request_id {uuid} NOT NULL UNIQUE REFERENCES sqb_role_activation_requests(request_id),
    location_id {uuid} NOT NULL,
    target TEXT NOT NULL,
    role_key TEXT NOT NULL,
    definition_digest TEXT NOT NULL,
    decision TEXT NOT NULL,
    decided_by TEXT NOT NULL,
    decided_at {time} NOT NULL,
    reason TEXT NOT NULL,
    signature TEXT,
    self_approval BOOLEAN NOT NULL
);
CREATE TABLE IF NOT EXISTS sqb_effective_roles (
    location_id {uuid} NOT NULL,
    target TEXT NOT NULL,
    role_key TEXT NOT NULL,
    definition_digest TEXT NOT NULL,
    resolved_grants_json TEXT NOT NULL,
    effective_since {time} NOT NULL,
    PRIMARY KEY (location_id, target, role_key)
);
