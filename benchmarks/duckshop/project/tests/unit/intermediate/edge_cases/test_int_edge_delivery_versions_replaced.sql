TEST (
  name "int_edge_delivery_versions_replaced__copies_every_version_of_shipments_touched_in_the_window",
  cursor_start "2026-03-02 00:00:00",
  cursor_end "2026-03-03 00:00:00",
);

WITH __source__edge__delivery_updates AS (
  SELECT
    v.delivery_id,
    v.shipment_id,
    v.order_ref,
    v.delivery_status,
    v.ordered_on::DATE AS ordered_on,
    v.delivered_on::DATE AS delivered_on,
    v._op,
    v._seq::NUMBER(38, 0) AS _seq,
    v._loaded_at::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('DU-01', 'SH-1', 'DS-O-3001', 'label_created', '2026-02-27', NULL, 'I', 10, '2026-03-01 10:00:00'),
    ('DU-04', 'SH-3', 'DS-O-3003', 'delivered', '2026-02-20', '2026-02-25', 'I', 30, '2026-03-01 10:00:00'),
    ('DU-06', 'SH-1', 'DS-O-3001', 'picked_up', '2026-02-27', NULL, 'U', 11, '2026-03-02 10:00:00'),
    ('DU-08', 'SH-2', 'DS-O-3002', 'delivered', '2026-02-28', '2026-03-02', 'U', 21, '2026-03-02 23:59:59'),
    ('DU-11', 'SH-6', 'DS-O-3006', 'label_created', '2026-03-02', NULL, 'I', 60, '2026-03-03 00:00:00') AS v(delivery_id, shipment_id, order_ref, delivery_status, ordered_on, delivered_on, _op, _seq, _loaded_at)
),

__ref__int_edge_delivery_update_versions AS (
  SELECT
    v.delivery_id,
    v.shipment_id,
    v.order_ref,
    v.delivery_status,
    v.ordered_on::DATE AS ordered_on,
    v.delivered_on::DATE AS delivered_on,
    v.is_delete,
    v.version_seq::NUMBER(38, 0) AS version_seq,
    v.loaded_at::TIMESTAMP_NTZ AS loaded_at
  FROM VALUES
    ('DU-01', 'SH-1', 'DS-O-3001', 'label_created', '2026-02-27', NULL, FALSE, 10, '2026-03-01 10:00:00'),
    ('DU-02', 'SH-1', 'DS-O-3001', 'in_transit', '2026-02-27', NULL, FALSE, 12, '2026-03-01 10:00:00'),
    ('DU-04', 'SH-3', 'DS-O-3003', 'delivered', '2026-02-20', '2026-02-25', FALSE, 30, '2026-03-01 10:00:00'),
    ('DU-06', 'SH-1', 'DS-O-3001', 'picked_up', '2026-02-27', NULL, FALSE, 11, '2026-03-02 10:00:00'),
    ('DU-08', 'SH-2', 'DS-O-3002', 'delivered', '2026-02-28', '2026-03-02', FALSE, 21, '2026-03-02 23:59:59'),
    ('DU-12', 'SH-1', 'DS-O-3001', 'delivered', '2026-02-27', '2026-03-03', FALSE, 13, '2026-03-03 00:00:00'),
    ('DU-11', 'SH-6', 'DS-O-3006', 'label_created', '2026-03-02', NULL, FALSE, 60, '2026-03-03 00:00:00') AS v(
    delivery_id,
    shipment_id,
    order_ref,
    delivery_status,
    ordered_on,
    delivered_on,
    is_delete,
    version_seq,
    loaded_at
  )
),

__expected__int_edge_delivery_versions_replaced AS (
  SELECT
    'DU-01' AS delivery_id,
    'SH-1' AS shipment_id,
    10 AS version_seq,
    '2026-03-02 10:00:00'::TIMESTAMP_NTZ(9) AS source_loaded_at
  UNION ALL
  SELECT
    'DU-02' AS delivery_id,
    'SH-1' AS shipment_id,
    12 AS version_seq,
    '2026-03-02 10:00:00'::TIMESTAMP_NTZ(9) AS source_loaded_at
  UNION ALL
  SELECT
    'DU-06' AS delivery_id,
    'SH-1' AS shipment_id,
    11 AS version_seq,
    '2026-03-02 10:00:00'::TIMESTAMP_NTZ(9) AS source_loaded_at
  UNION ALL
  SELECT
    'DU-08' AS delivery_id,
    'SH-2' AS shipment_id,
    21 AS version_seq,
    '2026-03-02 23:59:59'::TIMESTAMP_NTZ(9) AS source_loaded_at
)

SELECT 1
