CREATE TABLE sqb_admin_initialization (
    singleton BIGINT PRIMARY KEY CHECK (singleton=1),
    principal_id {uuid} NOT NULL,
    initialized_at {time} NOT NULL
);
CREATE TABLE sqb_role_assignments (
    assignment_id {uuid} PRIMARY KEY,
    principal_id {uuid} NOT NULL REFERENCES sqb_principals(principal_id),
    role_key TEXT NOT NULL,
    location_id {uuid} REFERENCES sqb_locations(location_id),
    target TEXT,
    source TEXT NOT NULL,
    granted_by TEXT NOT NULL,
    granted_at {time} NOT NULL,
    revoked_by TEXT,
    revoked_at {time},
    active_key TEXT UNIQUE
);
CREATE INDEX sqb_role_assignments_principal ON sqb_role_assignments(principal_id);
