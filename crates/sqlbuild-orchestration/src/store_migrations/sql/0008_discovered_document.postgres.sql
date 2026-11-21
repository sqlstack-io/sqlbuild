-- Snapshots keep what each @discover function returned in a new 'discovered' document.
ALTER TABLE sqb_snapshot_documents DROP CONSTRAINT sqb_snapshot_documents_kind_check;

ALTER TABLE sqb_snapshot_documents ADD CONSTRAINT sqb_snapshot_documents_kind_check CHECK (kind IN ('jobs', 'groups', 'triggers', 'roles', 'lineage', 'columns', 'providers', 'connection', 'discovered'));
