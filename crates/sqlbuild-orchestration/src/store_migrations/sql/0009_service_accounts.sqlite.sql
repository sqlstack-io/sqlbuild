-- Service accounts: principals that call with API keys instead of signing in. A key is stored only
-- as its SHA-256, and its prefix identifies it in lists and the audit log. IF NOT EXISTS lets the
-- migration run again over tables a store already has.
CREATE TABLE IF NOT EXISTS sqb_service_accounts (
    principal_id {uuid} PRIMARY KEY REFERENCES sqb_principals(principal_id),
    description TEXT,
    created_by TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sqb_service_account_keys (
    key_id {uuid} PRIMARY KEY,
    principal_id {uuid} NOT NULL REFERENCES sqb_principals(principal_id),
    secret_hash TEXT NOT NULL UNIQUE,
    prefix TEXT NOT NULL,
    origin TEXT NOT NULL CHECK (origin IN ('issued', 'imported')),
    created_at {time} NOT NULL,
    created_by TEXT NOT NULL,
    expires_at {time},
    revoked_at {time},
    last_used_at {time}
);
CREATE INDEX IF NOT EXISTS sqb_service_account_keys_principal ON sqb_service_account_keys(principal_id);
