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
