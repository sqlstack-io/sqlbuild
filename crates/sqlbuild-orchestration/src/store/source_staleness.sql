CREATE INDEX sqb_freshness_source_latest ON sqb_warehouse_freshness (
    location_id, target, (record_json ->> 'source_name'),
    (record_json ->> 'target_database'), (record_json ->> 'target_schema'),
    (record_json ->> 'target_name'), observed_at DESC, event_id DESC
);
