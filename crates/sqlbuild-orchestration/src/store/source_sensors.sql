CREATE INDEX recall_warehouse_sources ON recall_events (storage_id)
    WHERE producer='warehouse' AND event_type='warehouse_source_freshness'
      AND invocation_id LIKE 'warehouse/source-freshness/%';
