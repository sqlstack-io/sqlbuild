MODEL (
  description "One row per live shipment with tracking summary fields",
  columns (
    shipment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    carrier_code (type VARCHAR, audits [not_null]),
    service_level (type VARCHAR),
    tracking_number (type VARCHAR),
    shipped_at (type TIMESTAMP_TZ),
    delivered_at (type TIMESTAMP_TZ),
    tracking_event_count (type INT),
    tracking_events (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__shipments")
),

@sql.cdc_latest(src, src.key := (shipment_id))

final AS (
  SELECT
    s.shipment_id,
    s.order_id,
    UPPER(s.warehouse_code)::VARCHAR AS warehouse_code,
    UPPER(s.carrier_code)::VARCHAR AS carrier_code,
    LOWER(COALESCE(s.service_level, 'standard'))::VARCHAR AS service_level,
    UPPER(REGEXP_REPLACE(
      s.tracking:number::VARCHAR,
      '[^A-Za-z0-9]',
      ''
    ))::VARCHAR AS tracking_number,
    s.shipped_at,
    s.delivered_at,
    ARRAY_SIZE(COALESCE(s.tracking:events, ARRAY_CONSTRUCT()))::INT AS tracking_event_count,
    COALESCE(s.tracking:events, ARRAY_CONSTRUCT())::ARRAY AS tracking_events,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
)
SELECT *
FROM final
