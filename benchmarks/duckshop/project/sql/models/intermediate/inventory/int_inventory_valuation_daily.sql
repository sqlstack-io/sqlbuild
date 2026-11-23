MODEL (
  description """
    One row per warehouse and day with stock units and their value in USD at current supplier cost
  """,
  columns (
    valuation_key (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR, audits [not_null]),
    calendar_date (type DATE, audits [not_null]),
    sku_count (type INT),
    units_on_hand (type INT),
    stock_value_usd (type NUMBER(18, 2)),
    unvalued_units (type INT),
    stocked_out_skus (type INT),
    value_change_usd (type NUMBER(18, 2)),
  ),
);

WITH stock_days AS (
  SELECT *
  FROM __ref("int_stock_on_hand_daily")
),

costs AS (
  SELECT *
  FROM __ref("int_supplier_cost_current")
),

valued AS (
  SELECT
    s.warehouse_code,
    s.calendar_date,
    s.sku,
    s.on_hand_qty,
    s.is_stocked_out,
    s.on_hand_qty * c.unit_cost_usd AS stock_value_usd,
    IFF(c.unit_cost_usd IS NULL, s.on_hand_qty, 0) AS unvalued_units
  FROM stock_days AS s
  LEFT JOIN costs AS c
    ON s.sku = c.sku
),

daily AS (
  SELECT
    v.warehouse_code,
    v.calendar_date,
    COUNT(DISTINCT v.sku) AS sku_count,
    SUM(GREATEST(v.on_hand_qty, 0)) AS units_on_hand,
    SUM(GREATEST(v.stock_value_usd, 0)) AS stock_value_usd,
    SUM(v.unvalued_units) AS unvalued_units,
    COUNT_IF(v.is_stocked_out) AS stocked_out_skus
  FROM valued AS v
  GROUP BY
    v.warehouse_code,
    v.calendar_date
),

final AS (
  SELECT
    (
      d.warehouse_code || '|' || TO_CHAR(d.calendar_date, 'YYYY-MM-DD')
    )::VARCHAR AS valuation_key,
    d.warehouse_code,
    d.calendar_date,
    d.sku_count::INT AS sku_count,
    d.units_on_hand::INT AS units_on_hand,
    ROUND(d.stock_value_usd, 2)::NUMBER(18, 2) AS stock_value_usd,
    d.unvalued_units::INT AS unvalued_units,
    d.stocked_out_skus::INT AS stocked_out_skus,
    ROUND(
      d.stock_value_usd
        - LAG(d.stock_value_usd) OVER (PARTITION BY d.warehouse_code ORDER BY d.calendar_date),
      2
    )::NUMBER(18, 2) AS value_change_usd
  FROM daily AS d
)

SELECT *
FROM final
