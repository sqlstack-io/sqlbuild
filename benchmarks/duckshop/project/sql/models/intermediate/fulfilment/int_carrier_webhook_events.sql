MODEL (
  description """
    One row per carrier status update matched to a shipment by tracking number, keeping the first
    delivery of each carrier event id, with the receive lag.
  """,
  columns (
    carrier_update_key (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    carrier_code (type VARCHAR),
    status_code (type VARCHAR),
    status_at (type TIMESTAMP_TZ),
    received_at (type TIMESTAMP_NTZ),
    receive_lag_minutes (type INT),
    is_batched (type BOOLEAN),
  ),
);

WITH webhooks AS (
  SELECT *
  FROM __ref("stg_carrier_webhooks")
),

shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

first_delivery AS (
  SELECT
    w.carrier_update_key,
    w.carrier_code,
    w.tracking_number,
    w.status_code,
    w.status_at,
    w.received_at,
    w.is_batched
  FROM webhooks AS w
  WHERE w.tracking_number IS NOT NULL
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY w.carrier_code, COALESCE(w.carrier_event_id::VARCHAR, w.carrier_update_key)
      ORDER BY w.received_at, w.carrier_update_key
    ) = 1
),

final AS (
  SELECT
    f.carrier_update_key,
    s.shipment_id,
    f.carrier_code,
    f.status_code,
    f.status_at,
    f.received_at,
    DATEDIFF(
      'minute',
      CONVERT_TIMEZONE('UTC', f.status_at)::TIMESTAMP_NTZ,
      f.received_at
    )::INT AS receive_lag_minutes,
    f.is_batched
  FROM first_delivery AS f
  INNER JOIN shipments AS s
    ON f.tracking_number = s.tracking_number
    AND f.carrier_code = s.carrier_code
)

SELECT *
FROM final
