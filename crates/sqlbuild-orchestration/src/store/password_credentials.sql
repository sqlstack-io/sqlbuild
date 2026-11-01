CREATE TABLE sqb_password_credentials (
    principal_id {uuid} PRIMARY KEY REFERENCES sqb_principals(principal_id),
    credential_id {uuid} NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    changed_at {time} NOT NULL
);
