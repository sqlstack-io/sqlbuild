CREATE TABLE sqb_platform_sink_deliveries (
    sink_id TEXT NOT NULL,
    storage_id BIGINT NOT NULL,
    event_id TEXT NOT NULL,
    delivered_at {time} NOT NULL,
    PRIMARY KEY (sink_id, storage_id)
);
