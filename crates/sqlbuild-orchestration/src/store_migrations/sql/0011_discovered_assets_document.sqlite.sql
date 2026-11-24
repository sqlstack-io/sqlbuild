-- Snapshots name the for_each assets reading each discovered set in a new 'discovered_assets'
-- document.
CREATE TABLE sqb_snapshot_documents_v2 (
    snapshot_id {uuid} NOT NULL REFERENCES sqb_snapshots(snapshot_id),
    kind TEXT NOT NULL CHECK (kind IN ('jobs', 'groups', 'triggers', 'roles', 'lineage', 'columns', 'providers', 'connection', 'discovered', 'discovered_assets')),
    document_hash TEXT NOT NULL REFERENCES sqb_documents(document_hash),
    PRIMARY KEY (snapshot_id, kind)
);

INSERT INTO sqb_snapshot_documents_v2 (snapshot_id, kind, document_hash) SELECT snapshot_id, kind, document_hash FROM sqb_snapshot_documents;

DROP TABLE sqb_snapshot_documents;

ALTER TABLE sqb_snapshot_documents_v2 RENAME TO sqb_snapshot_documents;

CREATE INDEX IF NOT EXISTS sqb_snapshot_documents_hash ON sqb_snapshot_documents(document_hash);
