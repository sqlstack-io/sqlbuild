-- Historical credentials must not regain authority through rotation.
INSERT INTO sqb_locations (location_id, name, created_by, created_at, archived_at)
VALUES ('00000000-0000-4000-8000-000000000031', 'archived-orders', 'synthetic-operator', '2020-01-01T00:00:00Z', '2020-01-02T00:00:00Z');
INSERT INTO sqb_access_tokens
    (token_id, secret_hash, location_id, scope_json, created_at, expires_at)
VALUES (
    '00000000-0000-4000-8000-000000000032',
    '0000000000000000000000000000000000000000000000000000000000000000',
    '00000000-0000-4000-8000-000000000031',
    '{"kind":"location","location_id":"00000000-0000-4000-8000-000000000031"}',
    '2020-01-01T00:00:00Z',
    '2999-01-01T00:00:00Z'
);
