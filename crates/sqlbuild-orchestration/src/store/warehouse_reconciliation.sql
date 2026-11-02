ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN state_database TEXT;
ALTER TABLE sqb_warehouse_fingerprints ADD COLUMN state_schema TEXT;
UPDATE sqb_warehouse_fingerprints SET state_database=record_json ->> 'state_database', state_schema=record_json ->> 'state_schema';
CREATE INDEX sqb_fingerprints_reconciliation ON sqb_warehouse_fingerprints (location_id,target,state_database,state_schema,observed_at,event_id);
ALTER TABLE sqb_warehouse_freshness ADD COLUMN state_database TEXT;
ALTER TABLE sqb_warehouse_freshness ADD COLUMN state_schema TEXT;
UPDATE sqb_warehouse_freshness SET state_database=record_json ->> 'state_database', state_schema=record_json ->> 'state_schema';
CREATE INDEX sqb_freshness_reconciliation ON sqb_warehouse_freshness (location_id,target,state_database,state_schema,observed_at,event_id);
CREATE TABLE sqb_warehouse_scan_pages (
    scan_id {uuid} NOT NULL,
    page BIGINT NOT NULL,
    scope_hash TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    complete BOOLEAN NOT NULL,
    received_at {time} NOT NULL,
    PRIMARY KEY (scan_id,page)
);
CREATE TABLE sqb_warehouse_scan_keys (
    scan_id {uuid} NOT NULL,
    event_id {uuid} NOT NULL,
    PRIMARY KEY (scan_id,event_id)
);
