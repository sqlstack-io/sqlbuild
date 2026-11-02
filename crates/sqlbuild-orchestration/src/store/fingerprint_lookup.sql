ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN node_type TEXT;
ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN node_name TEXT;
ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN target_database TEXT;
ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN target_schema TEXT;
ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN target_name TEXT;
UPDATE sqb_warehouse_fingerprints SET
    node_type=record_json ->> 'node_type',
    node_name=record_json ->> 'node_name',
    target_database=record_json ->> 'target_database',
    target_schema=record_json ->> 'target_schema',
    target_name=record_json ->> 'target_name';
CREATE INDEX sqb_fingerprint_node_latest ON sqb_warehouse_fingerprints
    (location_id,target,node_type,node_name,target_database,target_schema,target_name,observed_at DESC,event_id DESC);
