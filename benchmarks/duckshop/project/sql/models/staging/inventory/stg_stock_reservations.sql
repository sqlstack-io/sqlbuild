MODEL (
  description "One row per live stock reservation held for an order",
  columns (
    reservation_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    qty (type INT),
    reserved_at (type TIMESTAMP_TZ),
    released_at (type TIMESTAMP_TZ),
    is_open (type BOOLEAN),
    allocation_rule (type VARCHAR),
    is_backorder (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__stock_reservations")
),

@sql.cdc_latest(src, src.key := (reservation_id))

final AS (
  SELECT
    r.reservation_id,
    r.order_id,
    @sql.normalise_sku(r.sku) AS sku,
    UPPER(r.warehouse_code)::VARCHAR AS warehouse_code,
    r.qty::INT AS qty,
    r.reserved_at,
    r.released_at,
    (
      r.released_at IS NULL
    ) AS is_open,
    LOWER(@sql.json_text(r.allocation:rule))::VARCHAR AS allocation_rule,
    COALESCE(r.allocation:backorder::BOOLEAN, FALSE) AS is_backorder,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
