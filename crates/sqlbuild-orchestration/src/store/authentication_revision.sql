CREATE TABLE sqb_authentication_revision (
    singleton BIGINT PRIMARY KEY CHECK (singleton = 1),
    revision BIGINT NOT NULL CHECK (revision >= 0)
);
INSERT INTO sqb_authentication_revision (singleton, revision) VALUES (1, 0);
