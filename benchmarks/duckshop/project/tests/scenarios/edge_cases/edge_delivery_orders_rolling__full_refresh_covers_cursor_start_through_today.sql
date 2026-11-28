SCENARIO (
  description """
    A full refresh of the rolling-window order model covers every day from its cursor_start
    through the current UTC day, today's orders included; orders from before cursor_start and
    orders dated in the future are left out
  """,
  tags ["edge_case"],
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
    ('DR-01', 'SH-1', 'DR-O-4001', 'label_created', '2026-01-31', NULL, 'I', 10, '2026-01-31 00:00:00'),
    ('DR-02', 'SH-2', 'DR-O-4002', 'label_created', '2026-02-01', NULL, 'I', 20, '2026-02-01 00:00:00'),
    ('DR-03', 'SH-2', 'DR-O-4002', 'delivered', '2026-02-01', '2026-02-04', 'U', 21, '2026-02-04 00:00:00'),
    ('DR-04', 'SH-3', 'DR-O-4003', 'label_created', '2026-03-15', NULL, 'I', 30, '2026-03-15 00:00:00'),
    ('DR-05', 'SH-4', 'DR-O-4004', 'label_created', '2999-12-29', NULL, 'I', 40, '2999-12-29 00:00:00') AS v(delivery_id, shipment_id, order_ref, delivery_status, ordered_on, delivered_on, _op, _seq, _loaded_at)
  UNION ALL
  SELECT
    'DR-06' AS delivery_id,
    'SH-5' AS shipment_id,
    'DR-O-4005' AS order_ref,
    'label_created' AS delivery_status,
    CURRENT_DATE AS ordered_on,
    NULL::DATE AS delivered_on,
    'I' AS _op,
    50::NUMBER(38, 0) AS _seq,
    CURRENT_DATE::TIMESTAMP_NTZ AS _loaded_at
),

__expected__int_edge_delivery_orders_rolling AS (
  SELECT
    v.delivery_id,
    v.shipment_id,
    v.ordered_on::DATE AS ordered_on
  FROM VALUES
    ('DR-02', 'SH-2', '2026-02-01'),
    ('DR-03', 'SH-2', '2026-02-01'),
    ('DR-04', 'SH-3', '2026-03-15') AS v(delivery_id, shipment_id, ordered_on)
  UNION ALL
  SELECT
    'DR-06' AS delivery_id,
    'SH-5' AS shipment_id,
    CURRENT_DATE AS ordered_on
)
