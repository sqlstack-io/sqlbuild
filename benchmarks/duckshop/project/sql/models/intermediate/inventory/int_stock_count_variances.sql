MODEL (
  description """
    One row per cycle count with the quantity the ledger expected at that moment (ASOF the ledger),
    the variance, and whether it exceeds the warehouse's tolerance.
  """,
  constants (
    _tolerance_units 2,
    _tolerance_pct constant(type decimal, value "0.05"),
  ),
  columns (
    count_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    counted_at_local (type TIMESTAMP_NTZ),
    counted_qty (type INT),
    expected_qty (type INT),
    variance_qty (type INT),
    variance_pct (type NUMBER(9, 4)),
    is_out_of_tolerance (type BOOLEAN),
    is_blind_count (type BOOLEAN),
    recount_sequence (type INT),
  ),
);

WITH counts AS (
  SELECT *
  FROM __ref("stg_stock_counts")
),

ledger AS (
  SELECT *
  FROM __ref("int_inventory_ledger")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

counts_utc AS (
  SELECT
    c.count_id,
    c.sku,
    c.warehouse_code,
    c.counted_at_local,
    c.counted_qty,
    c.is_blind_count,
    CONVERT_TIMEZONE(w.iana_timezone, 'UTC', c.counted_at_local)::TIMESTAMP_NTZ AS counted_at_utc
  FROM counts AS c
  INNER JOIN warehouses AS w
    ON c.warehouse_code = w.warehouse_code
),

with_expected AS (
  SELECT
    c.count_id,
    c.sku,
    c.warehouse_code,
    c.counted_at_local,
    c.counted_qty,
    c.is_blind_count,
    COALESCE(l.running_on_hand, 0) AS expected_qty
  FROM counts_utc AS c
  ASOF JOIN ledger AS l MATCH_CONDITION (c.counted_at_utc >= l.occurred_at_utc)
    ON c.sku = l.sku
    AND c.warehouse_code = l.warehouse_code
),

final AS (
  SELECT
    e.count_id,
    e.sku,
    e.warehouse_code,
    e.counted_at_local,
    e.counted_qty,
    e.expected_qty::INT AS expected_qty,
    (
      e.counted_qty - e.expected_qty
    )::INT AS variance_qty,
    ROUND((
      e.counted_qty - e.expected_qty
    )
    / NULLIF(e.expected_qty, 0), 4)::NUMBER(9, 4) AS variance_pct,
    (
      ABS(e.counted_qty - e.expected_qty) > @const("_tolerance_units")
      AND ABS(
        e.counted_qty - e.expected_qty
      ) > @const("_tolerance_pct") * GREATEST(e.expected_qty, 1)
    ) AS is_out_of_tolerance,
    e.is_blind_count,
    COUNT(*) OVER (
      PARTITION BY e.sku, e.warehouse_code, e.counted_at_local::DATE
      ORDER BY e.counted_at_local, e.count_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::INT AS recount_sequence
  FROM with_expected AS e
)

SELECT *
FROM final
