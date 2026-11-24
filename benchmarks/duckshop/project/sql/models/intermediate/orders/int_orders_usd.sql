MODEL (
  description """
    One row per order with amounts converted to major units and to USD at the latest reference rate
    on or before the local order date (an ASOF join against daily rates).
  """,
  audits [
    match_rate (
      name "fx_rate_found",
      matched "rate_to_usd IS NOT NULL",
      minimum_samples 50,
      evidence_limit 20,
      thresholds (warn (below 99.5), error (below 95)),
    ),
  ],
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR),
    region_code (type VARCHAR),
    currency_code (type VARCHAR),
    placed_at (type TIMESTAMP_TZ),
    placed_date_local (type DATE),
    order_status (type VARCHAR),
    sales_channel (type VARCHAR),
    units (type INT),
    amount (type NUMBER(38, 6)),
    amount_usd (type NUMBER(18, 2)),
    gross_amount_usd (type NUMBER(18, 2)),
    discount_amount_usd (type NUMBER(18, 2)),
    tax_amount_usd (type NUMBER(18, 2)),
    rate_to_usd (type NUMBER(18, 8)),
    rate_date (type DATE),
    rate_age_days (type INT),
    is_rate_stale (type BOOLEAN),
    promo_code (type VARCHAR),
    utm_source (type VARCHAR),
    utm_medium (type VARCHAR),
    utm_campaign (type VARCHAR),
    basket_size (type VARCHAR),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("int_orders_enriched")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

orders_with_exponent AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.region_code,
    o.currency_code,
    o.placed_at,
    o.placed_date_local,
    o.order_status,
    o.sales_channel,
    o.units,
    o.net_amount_minor AS amount_minor,
    o.gross_amount_minor,
    o.discount_amount_minor,
    o.tax_amount_minor,
    o.promo_code,
    o.utm_source,
    o.utm_medium,
    o.utm_campaign,
    o.basket_size,
    c.minor_unit_exponent
  FROM orders AS o
  INNER JOIN currencies AS c
    ON o.currency_code = c.currency_code
),

orders_with_rate AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.region_code,
    o.currency_code,
    o.placed_at,
    o.placed_date_local,
    o.order_status,
    o.sales_channel,
    o.units,
    o.amount_minor,
    o.gross_amount_minor,
    o.discount_amount_minor,
    o.tax_amount_minor,
    o.promo_code,
    o.utm_source,
    o.utm_medium,
    o.utm_campaign,
    o.basket_size,
    o.minor_unit_exponent,
    fx.rate_to_usd,
    fx.rate_date
  FROM orders_with_exponent AS o
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (o.placed_date_local >= fx.rate_date)
    ON o.currency_code = fx.currency_code
),

final AS (
  SELECT
    r.order_id,
    r.customer_id,
    r.region_code,
    r.currency_code,
    r.placed_at,
    r.placed_date_local,
    r.order_status,
    r.sales_channel,
    r.units,
    @sql.money_columns(r),
    ROUND(
      @sql.money_from_minor(r.gross_amount_minor, r.minor_unit_exponent, places := 6) * r.rate_to_usd,
      2
    )::NUMBER(18, 2) AS gross_amount_usd,
    ROUND(
      @sql.money_from_minor(r.discount_amount_minor, r.minor_unit_exponent, places := 6) * r.rate_to_usd,
      2
    )::NUMBER(18, 2) AS discount_amount_usd,
    ROUND(
      @sql.money_from_minor(r.tax_amount_minor, r.minor_unit_exponent, places := 6) * r.rate_to_usd,
      2
    )::NUMBER(18, 2) AS tax_amount_usd,
    r.rate_to_usd,
    r.rate_date,
    DATEDIFF('day', r.rate_date, r.placed_date_local)::INT AS rate_age_days,
    (
      DATEDIFF('day', r.rate_date, r.placed_date_local) > 7
    ) AS is_rate_stale,
    r.promo_code,
    r.utm_source,
    r.utm_medium,
    r.utm_campaign,
    r.basket_size
  FROM orders_with_rate AS r
)

SELECT *
FROM final
