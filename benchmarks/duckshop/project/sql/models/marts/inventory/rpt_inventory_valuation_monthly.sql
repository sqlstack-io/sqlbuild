MODEL (
  description """
    One row per warehouse and month-end with stock value in USD, its monthly change and unvalued
    units
  """,
  columns (
    valuation_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR),
    month_end (type DATE),
    stock_value_usd (type NUMBER(18, 2)),
    previous_month_value_usd (type NUMBER(18, 2)),
    change_pct (type NUMBER(9, 2)),
    units_on_hand (type INT),
    unvalued_units (type INT),
    stocked_out_skus (type INT),
  ),
);

WITH valuation AS (
  SELECT *
  FROM __ref("int_inventory_valuation_daily")
),

month_ends AS (
  SELECT
    v.warehouse_code,
    v.calendar_date AS month_end,
    v.stock_value_usd,
    v.units_on_hand,
    v.unvalued_units,
    v.stocked_out_skus
  FROM valuation AS v
  WHERE
    v.calendar_date = LAST_DAY(v.calendar_date)
    OR v.calendar_date = '@@valuation_date'::DATE
),

final AS (
  SELECT
    (
      m.warehouse_code || ':' || TO_VARCHAR(m.month_end, 'YYYY-MM-DD')
    )::VARCHAR AS valuation_month_key,
    m.warehouse_code,
    m.month_end,
    m.stock_value_usd,
    LAG(m.stock_value_usd) OVER (
      PARTITION BY m.warehouse_code
      ORDER BY m.month_end
    )::NUMBER(18, 2) AS previous_month_value_usd,
    ROUND(
      100
      * (
        m.stock_value_usd
        / NULLIF(
          LAG(m.stock_value_usd) OVER (PARTITION BY m.warehouse_code ORDER BY m.month_end),
          0
        )
        - 1
      ),
      2
    )::NUMBER(9, 2) AS change_pct,
    m.units_on_hand,
    m.unvalued_units,
    m.stocked_out_skus
  FROM month_ends AS m
)

SELECT *
FROM final
