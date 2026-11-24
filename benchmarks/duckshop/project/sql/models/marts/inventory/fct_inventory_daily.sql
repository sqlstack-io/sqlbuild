MODEL (
  description """
    One row per SKU, warehouse and calendar day with stock on hand, its USD value at current
    supplier cost, and stock-out and count-age flags. Rebuilt incrementally by day with a three-day
    lookback for late stock movements.
  """,
  materialized incremental,
  incremental_strategy delete_insert,
  unique_key [stock_day_key],
  cursor calendar_date,
  cursor_type timestamp,
  cursor_grain day,
  lookback 3d,
  cursor_inputs (
    int_stock_on_hand_daily calendar_date,
  ),
  replay_on_change bounded-120d,
  on_schema_change append_new_columns,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    stock_day_key (type VARCHAR, nullable false, audits [
      not_null (run_scope delta_and_final),
      unique,
    ]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    calendar_date (type DATE, audits [not_null]),
    on_hand_qty (type INT),
    unit_cost_usd (type NUMBER(18, 4)),
    stock_value_usd (type NUMBER(18, 2)),
    is_stocked_out (type BOOLEAN),
    days_since_count (type INT),
    is_count_overdue (type BOOLEAN),
  ),
);

WITH stock AS (
  SELECT *
  FROM __ref("int_stock_on_hand_daily")
),

costs AS (
  SELECT *
  FROM __ref("int_supplier_cost_current")
),

windowed AS (
  SELECT
    s.stock_day_key,
    s.sku,
    s.warehouse_code,
    s.calendar_date,
    s.on_hand_qty,
    s.is_stocked_out,
    s.days_since_count
  FROM stock AS s
  WHERE
    s.calendar_date >= __cursor_start()
    AND s.calendar_date < __cursor_end()
),

final AS (
  SELECT
    w.stock_day_key,
    w.sku,
    w.warehouse_code,
    w.calendar_date,
    w.on_hand_qty,
    c.unit_cost_usd,
    ROUND(GREATEST(w.on_hand_qty, 0) * c.unit_cost_usd, 2)::NUMBER(18, 2) AS stock_value_usd,
    w.is_stocked_out,
    w.days_since_count,
    COALESCE(w.days_since_count > 90, TRUE)::BOOLEAN AS is_count_overdue
  FROM windowed AS w
  LEFT JOIN costs AS c
    ON w.sku = c.sku
)

SELECT *
FROM final
