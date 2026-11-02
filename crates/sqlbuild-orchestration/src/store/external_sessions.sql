CREATE TABLE sqb_external_sessions (
    session_id {uuid} PRIMARY KEY,
    principal_id {uuid} NOT NULL REFERENCES sqb_principals(principal_id),
    secret_hash TEXT NOT NULL,
    csrf_hash TEXT NOT NULL,
    created_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    revoked_at {time}
);
CREATE INDEX sqb_external_sessions_person ON sqb_external_sessions(principal_id);
