-- Synthetic predecessor rows exercise rejection without deleting credential history.
INSERT INTO sqb_principals (principal_id, name, kind, created_at)
VALUES ('00000000-0000-4000-8000-000000000001', 'orders-operator', 'person', '2026-01-01T00:00:00Z');
INSERT INTO sqb_access_tokens
    (token_id, secret_hash, principal_id, scope_json, created_at, expires_at)
VALUES (
    '00000000-0000-4000-8000-000000000002',
    '17a07b942485f9d9e9a230b900005ceec1f6b4864574bd19783165c9c7ca1da3',
    '00000000-0000-4000-8000-000000000001',
    '{"kind":"principal","principal_id":"00000000-0000-4000-8000-000000000001"}',
    '2026-01-01T00:00:00Z',
    '2999-01-01T00:00:00Z'
);
