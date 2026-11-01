CREATE TABLE sqb_password_sessions (
    session_id {uuid} PRIMARY KEY,
    principal_id {uuid} NOT NULL REFERENCES sqb_principals(principal_id),
    credential_id {uuid} NOT NULL,
    secret_hash TEXT NOT NULL,
    csrf_hash TEXT NOT NULL,
    created_at {time} NOT NULL,
    expires_at {time} NOT NULL,
    revoked_at {time}
);
CREATE INDEX sqb_password_sessions_person ON sqb_password_sessions(principal_id);
CREATE TABLE sqb_password_login_attempts (
    attempt_id {uuid} PRIMARY KEY,
    identity_key TEXT NOT NULL,
    peer_address TEXT NOT NULL,
    attempted_at {time} NOT NULL
);
CREATE INDEX sqb_password_login_user_time ON sqb_password_login_attempts(identity_key, attempted_at);
CREATE INDEX sqb_password_login_peer_time ON sqb_password_login_attempts(peer_address, attempted_at);
CREATE INDEX sqb_password_login_time ON sqb_password_login_attempts(attempted_at);
