TEST (
  name "int_edge_delivery_status_replaced__copies_only_shipments_loaded_in_the_window",
  cursor_start "2026-03-02 00:00:00",
  cursor_end "2026-03-03 00:00:00",
);

WITH __ref__int_edge_delivery_status AS (
  SELECT
    v.shipment_id,
    v.order_ref,
    v.delivery_status,
    v.ordered_on::DATE AS ordered_on,
    v.delivered_on::DATE AS delivered_on,
    v.is_deleted,
    v.latest_seq::NUMBER(38, 0) AS latest_seq,
    v.source_loaded_at::TIMESTAMP_NTZ AS source_loaded_at
  FROM VALUES
    ('SH-1', 'DS-O-3001', 'in_transit', '2026-02-27', NULL, FALSE, 12, '2026-03-02 10:00:00'),
    ('SH-3', 'DS-O-3003', 'delivered', '2026-02-20', '2026-02-25', FALSE, 30, '2026-03-01 10:00:00'),
    ('SH-4', 'DS-O-3004', 'in_transit', '2026-02-26', NULL, TRUE, 41, '2026-03-02 23:59:59'),
    ('SH-6', 'DS-O-3006', 'label_created', '2026-03-02', NULL, FALSE, 60, '2026-03-03 00:00:00') AS v(
    shipment_id,
    order_ref,
    delivery_status,
    ordered_on,
    delivered_on,
    is_deleted,
    latest_seq,
    source_loaded_at
  )
),

__expected__int_edge_delivery_status_replaced AS (
  SELECT
    'SH-1' AS shipment_id,
    'in_transit' AS delivery_status,
    FALSE AS is_deleted,
    12 AS latest_seq,
    '2026-03-02 10:00:00'::TIMESTAMP_NTZ(9) AS source_loaded_at
  UNION ALL
  SELECT
    'SH-4' AS shipment_id,
    'in_transit' AS delivery_status,
    TRUE AS is_deleted,
    41 AS latest_seq,
    '2026-03-02 23:59:59'::TIMESTAMP_NTZ(9) AS source_loaded_at
)

SELECT 1
