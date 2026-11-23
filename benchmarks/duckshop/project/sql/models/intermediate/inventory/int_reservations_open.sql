MODEL (
  description """
    One row per SKU and warehouse with open reservations against the latest on-hand quantity, the
    available-to-promise quantity and the oldest open reservation.
  """,
  columns (
    stock_key (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    open_reservations (type INT),
    reserved_qty (type INT),
    backordered_qty (type INT),
    on_hand_qty (type INT),
    available_to_promise (type INT),
    oldest_reserved_at (type TIMESTAMP_TZ),
  ),
);

WITH reservations AS (
  SELECT *
  FROM __ref("stg_stock_reservations")
),

ledger AS (
  SELECT *
  FROM __ref("int_inventory_ledger")
),

latest_balance AS (
  SELECT
    l.sku,
    l.warehouse_code,
    l.running_on_hand
  FROM ledger AS l
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY l.sku, l.warehouse_code
      ORDER BY l.entry_sequence DESC, l.ledger_entry_key DESC
    ) = 1
),

open_holds AS (
  SELECT
    r.sku,
    r.warehouse_code,
    COUNT(*) AS open_reservations,
    SUM(IFF(r.is_backorder, 0, r.qty)) AS reserved_qty,
    SUM(IFF(r.is_backorder, r.qty, 0)) AS backordered_qty,
    MIN(r.reserved_at) AS oldest_reserved_at
  FROM reservations AS r
  WHERE r.is_open
  GROUP BY
    r.sku,
    r.warehouse_code
),

final AS (
  SELECT
    (
      o.sku || '|' || o.warehouse_code
    )::VARCHAR AS stock_key,
    o.sku,
    o.warehouse_code,
    o.open_reservations::INT AS open_reservations,
    o.reserved_qty::INT AS reserved_qty,
    o.backordered_qty::INT AS backordered_qty,
    COALESCE(b.running_on_hand, 0)::INT AS on_hand_qty,
    (
      COALESCE(b.running_on_hand, 0) - o.reserved_qty
    )::INT AS available_to_promise,
    o.oldest_reserved_at
  FROM open_holds AS o
  LEFT JOIN latest_balance AS b
    ON o.sku = b.sku
    AND o.warehouse_code = b.warehouse_code
)

SELECT *
FROM final
