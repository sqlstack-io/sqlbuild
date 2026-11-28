TEST (
  name "int_edge_delivery_orders_rolling__keeps_orders_inside_the_batch_days",
  cursor_start "2026-03-14",
  cursor_end "2026-03-16",
);

WITH __source__edge__delivery_updates AS (
  SELECT
    v.delivery_id,
    v.shipment_id,
    NULL AS order_ref,
    NULL AS delivery_status,
    v.ordered_on::DATE AS ordered_on,
    NULL::DATE AS delivered_on,
    'I' AS _op,
    v._seq::NUMBER(38, 0) AS _seq,
    '2026-03-16 10:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('DR-01', 'SH-1', '2026-03-13', 1),
    ('DR-02', 'SH-2', '2026-03-14', 2),
    ('DR-03', 'SH-3', '2026-03-15', 3),
    ('DR-04', 'SH-4', '2026-03-16', 4) AS v(delivery_id, shipment_id, ordered_on, _seq)
),

__expected__int_edge_delivery_orders_rolling AS (
  SELECT
    'DR-02' AS delivery_id,
    'SH-2' AS shipment_id,
    '2026-03-14'::DATE AS ordered_on
  UNION ALL
  SELECT
    'DR-03' AS delivery_id,
    'SH-3' AS shipment_id,
    '2026-03-15'::DATE AS ordered_on
)
