MODEL (
  description """
    One row per SKU and currency with its current list price, landed cost in the same currency, unit
    margin and margin percentage.
  """,
  columns (
    sku_price_key (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    list_price (type NUMBER(18, 4)),
    unit_cost (type NUMBER(18, 4)),
    unit_margin (type NUMBER(18, 4)),
    margin_pct (type NUMBER(9, 2)),
    is_below_cost (type BOOLEAN),
    category_level_1 (type VARCHAR),
  ),
);

WITH catalogue AS (
  SELECT *
  FROM __ref("int_product_catalogue")
),

price_history AS (
  SELECT *
  FROM __ref("int_price_history")
),

costs AS (
  SELECT *
  FROM __ref("int_supplier_cost_current")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

current_prices AS (
  SELECT
    ph.sku,
    ph.currency_code,
    ph.list_amount_minor,
    '@@valuation_date'::DATE AS valuation_date
  FROM price_history AS ph
  WHERE ph.effective_valid_to IS NULL
),

with_exponent AS (
  SELECT
    cp.sku,
    cp.currency_code,
    cp.list_amount_minor,
    cp.valuation_date,
    c.minor_unit_exponent
  FROM current_prices AS cp
  INNER JOIN currencies AS c
    ON cp.currency_code = c.currency_code
),

with_rate AS (
  SELECT
    w.sku,
    w.currency_code,
    w.list_amount_minor,
    w.minor_unit_exponent,
    fx.usd_to_rate
  FROM with_exponent AS w
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.valuation_date >= fx.rate_date)
    ON w.currency_code = fx.currency_code
),

final AS (
  SELECT
    (
      r.sku || ':' || r.currency_code
    )::VARCHAR AS sku_price_key,
    r.sku,
    r.currency_code,
    @sql.money_from_minor(r.list_amount_minor, r.minor_unit_exponent, places := 4)::NUMBER(18, 4) AS list_price,
    ROUND(c.unit_cost_usd * r.usd_to_rate, 4)::NUMBER(18, 4) AS unit_cost,
    (
      @sql.money_from_minor(r.list_amount_minor, r.minor_unit_exponent, places := 4)
        - ROUND(c.unit_cost_usd * r.usd_to_rate, 4)
    )::NUMBER(18, 4) AS unit_margin,
    ROUND(
      100
      * (
        1
          - ROUND(c.unit_cost_usd * r.usd_to_rate, 4)
          / NULLIF(
            @sql.money_from_minor(r.list_amount_minor, r.minor_unit_exponent, places := 4),
            0
          )
      ),
      2
    )::NUMBER(9, 2) AS margin_pct,
    COALESCE(
      ROUND(
        c.unit_cost_usd * r.usd_to_rate,
        4
      ) > @sql.money_from_minor(r.list_amount_minor, r.minor_unit_exponent, places := 4),
      FALSE
    ) AS is_below_cost,
    cat.category_level_1
  FROM with_rate AS r
  LEFT JOIN costs AS c
    ON r.sku = c.sku
  LEFT JOIN catalogue AS cat
    ON r.sku = cat.sku
)

SELECT *
FROM final
