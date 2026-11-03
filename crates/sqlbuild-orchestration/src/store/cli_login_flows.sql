CREATE TABLE sqb_cli_login_flows (
    flow_id {uuid} PRIMARY KEY,
    client_hash TEXT NOT NULL,
    browser_hash TEXT NOT NULL,
    expires_at {time} NOT NULL,
    session_id {uuid}
);
CREATE INDEX sqb_cli_login_expiry ON sqb_cli_login_flows(expires_at);
