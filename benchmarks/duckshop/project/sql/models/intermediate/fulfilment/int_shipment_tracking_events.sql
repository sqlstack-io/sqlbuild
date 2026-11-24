MODEL (
  description """
    One row per tracking event of a shipment from carrier scans, recorded exceptions and webhooks,
    deduplicated by code and minute, in UTC and in the shipping warehouse's local time.
  """,
  columns (
    tracking_event_key (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    event_code (type VARCHAR),
    event_family (type VARCHAR),
    event_at_utc (type TIMESTAMP_NTZ),
    event_at_warehouse_local (type TIMESTAMP_NTZ),
    event_origin (type VARCHAR),
    is_delivery_event (type BOOLEAN),
  ),
);

WITH tracking_events AS (
  SELECT *
  FROM __ref("stg_shipment_tracking_events")
),

webhook_events AS (
  SELECT *
  FROM __ref("int_carrier_webhook_events")
),

shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

combined AS (
  SELECT
    t.tracking_event_key::VARCHAR AS tracking_event_key,
    t.shipment_id::VARCHAR AS shipment_id,
    t.event_code::VARCHAR AS event_code,
    CONVERT_TIMEZONE('UTC', t.event_at)::TIMESTAMP_NTZ AS event_at_utc,
    t.event_source::VARCHAR AS event_origin
  FROM tracking_events AS t
  WHERE t.event_at IS NOT NULL
  UNION ALL
  SELECT
    (
      'W:' || w.carrier_update_key
    )::VARCHAR AS tracking_event_key,
    w.shipment_id::VARCHAR AS shipment_id,
    w.status_code::VARCHAR AS event_code,
    CONVERT_TIMEZONE('UTC', w.status_at)::TIMESTAMP_NTZ AS event_at_utc,
    'webhook'::VARCHAR AS event_origin
  FROM webhook_events AS w
  WHERE w.status_at IS NOT NULL
),

deduplicated AS (
  SELECT
    c.tracking_event_key,
    c.shipment_id,
    c.event_code,
    c.event_at_utc,
    c.event_origin
  FROM combined AS c
  QUALIFY
    COUNT(*) OVER (
      PARTITION BY c.shipment_id, c.event_code, DATE_TRUNC('minute', c.event_at_utc)
      ORDER BY c.event_origin, c.tracking_event_key
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) = 1
),

final AS (
  SELECT
    d.tracking_event_key,
    d.shipment_id,
    d.event_code,
    CASE
      WHEN d.event_code RLIKE '^(DELIVERED|DLV|POD).*'
      THEN 'delivered'
      WHEN d.event_code RLIKE '^(OUT_FOR_DELIVERY|OFD).*'
      THEN 'out_for_delivery'
      WHEN d.event_code RLIKE '^EXCEPTION_.*|^(EXC|DELAY|DAMAGE).*'
      THEN 'exception'
      WHEN d.event_code RLIKE '^(PICKED_UP|PU|ACCEPTED).*'
      THEN 'collected'
      ELSE 'in_transit'
    END::VARCHAR AS event_family,
    d.event_at_utc,
    CONVERT_TIMEZONE(
      'UTC',
      w.iana_timezone,
      d.event_at_utc
    )::TIMESTAMP_NTZ AS event_at_warehouse_local,
    d.event_origin,
    (
      d.event_code RLIKE '^(DELIVERED|DLV|POD).*'
    ) AS is_delivery_event
  FROM deduplicated AS d
  INNER JOIN shipments AS s
    ON d.shipment_id = s.shipment_id
  INNER JOIN warehouses AS w
    ON s.warehouse_code = w.warehouse_code
)

SELECT *
FROM final
