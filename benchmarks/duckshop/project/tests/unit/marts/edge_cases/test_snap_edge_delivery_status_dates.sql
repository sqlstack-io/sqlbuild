TEST (name "snap_edge_delivery_status_dates__versions_shipments_by_load_day");

WITH __ref__int_edge_delivery_status AS (
  SELECT
    'SH-1' AS shipment_id,
    'DS-O-3001' AS order_ref,
    'delivered' AS delivery_status,
    '1899-12-30'::DATE AS ordered_on,
    '2999-12-29'::DATE AS delivered_on,
    FALSE AS is_deleted,
    12::NUMBER(38, 0) AS latest_seq,
    '2999-12-30 23:59:59'::TIMESTAMP_NTZ AS source_loaded_at
  UNION ALL
  SELECT
    'SH-2' AS shipment_id,
    'DS-O-3002' AS order_ref,
    'label_created' AS delivery_status,
    '2026-03-01'::DATE AS ordered_on,
    CAST(NULL AS DATE) AS delivered_on,
    TRUE AS is_deleted,
    21::NUMBER(38, 0) AS latest_seq,
    '1900-01-01 00:00:00'::TIMESTAMP_NTZ AS source_loaded_at
),

__expected__snap_edge_delivery_status_dates AS (
  SELECT
    'SH-1' AS shipment_id,
    'delivered' AS delivery_status,
    FALSE AS is_deleted,
    '2999-12-30'::DATE AS loaded_on
  UNION ALL
  SELECT
    'SH-2' AS shipment_id,
    'label_created' AS delivery_status,
    TRUE AS is_deleted,
    '1900-01-01'::DATE AS loaded_on
)
