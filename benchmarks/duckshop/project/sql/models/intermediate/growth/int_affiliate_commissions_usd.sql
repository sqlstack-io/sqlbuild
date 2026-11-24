MODEL (
  description """
    One row per affiliate conversion with the commission in USD, the order it claims, whether it is
    the last-click conversion that earns the commission, and whether the order was later cancelled
    or returned so the commission should be reversed.
  """,
  columns (
    conversion_id (type VARCHAR, nullable false, audits [not_null, unique]),
    affiliate_code (type VARCHAR, audits [not_null]),
    affiliate_network (type VARCHAR),
    sub_channel (type VARCHAR),
    order_id (type VARCHAR),
    converted_at (type TIMESTAMP_TZ),
    click_to_conversion_hours (type INT),
    commission (type NUMBER(38, 6)),
    commission_usd (type NUMBER(18, 2)),
    order_amount_usd (type NUMBER(18, 2)),
    commission_rate (type NUMBER(9, 6)),
    claims_on_order (type INT),
    is_last_click (type BOOLEAN),
    is_reversible (type BOOLEAN),
    payable_commission_usd (type NUMBER(18, 2)),
  ),
);

WITH conversions AS (
  SELECT *
  FROM __ref("stg_affiliate_conversions")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

ranked_claims AS (
  SELECT
    c.conversion_id,
    c.affiliate_code,
    c.order_id,
    c.commission_minor,
    c.currency_code,
    c.converted_at,
    CONVERT_TIMEZONE('UTC', c.converted_at)::DATE AS converted_date,
    c.click_to_conversion_hours,
    c.sub_id,
    ROW_NUMBER() OVER (
      PARTITION BY c.order_id
      ORDER BY c.clicked_at DESC NULLS LAST, c.conversion_id
    ) AS claim_rank,
    COUNT(*) OVER (PARTITION BY c.order_id) AS claims_on_order
  FROM conversions AS c
),

parsed_codes AS (
  SELECT
    r.conversion_id,
    UPPER(
      COALESCE(REGEXP_SUBSTR(r.affiliate_code, '^([A-Za-z]{2,5})[-_]', 1, 1, 'e', 1), 'DIRECT')
    ) AS affiliate_network,
    LOWER(REGEXP_REPLACE(
      REGEXP_SUBSTR(r.sub_id, '^[a-zA-Z]+'),
      '[^a-z]',
      ''
    )) AS sub_channel
  FROM ranked_claims AS r
),

with_exponent AS (
  SELECT
    r.conversion_id,
    r.affiliate_code,
    r.order_id,
    r.commission_minor,
    r.currency_code,
    r.converted_at,
    r.converted_date,
    r.click_to_conversion_hours,
    r.claim_rank,
    r.claims_on_order,
    cur.minor_unit_exponent
  FROM ranked_claims AS r
  LEFT JOIN currencies AS cur
    ON r.currency_code = cur.currency_code
),

with_rate AS (
  SELECT
    w.conversion_id,
    w.affiliate_code,
    w.order_id,
    w.commission_minor,
    w.converted_at,
    w.click_to_conversion_hours,
    w.claim_rank,
    w.claims_on_order,
    w.minor_unit_exponent,
    fx.rate_to_usd
  FROM with_exponent AS w
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.converted_date >= fx.rate_date)
    ON w.currency_code = fx.currency_code
),

with_order AS (
  SELECT
    w.conversion_id,
    w.affiliate_code,
    p.affiliate_network,
    p.sub_channel,
    w.order_id,
    w.converted_at,
    w.click_to_conversion_hours,
    @sql.money_from_minor(w.commission_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) AS commission,
    w.rate_to_usd,
    w.claim_rank,
    w.claims_on_order,
    o.amount_usd AS order_amount_usd,
    o.order_status
  FROM with_rate AS w
  INNER JOIN parsed_codes AS p
    ON w.conversion_id = p.conversion_id
  LEFT JOIN orders AS o
    ON w.order_id = o.order_id
),

final AS (
  SELECT
    o.conversion_id,
    o.affiliate_code,
    o.affiliate_network::VARCHAR AS affiliate_network,
    NULLIF(o.sub_channel, '')::VARCHAR AS sub_channel,
    o.order_id,
    o.converted_at,
    o.click_to_conversion_hours,
    o.commission::NUMBER(38, 6) AS commission,
    ROUND(o.commission * o.rate_to_usd, 2)::NUMBER(18, 2) AS commission_usd,
    o.order_amount_usd,
    ROUND(
      o.commission * o.rate_to_usd / NULLIF(o.order_amount_usd, 0),
      6
    )::NUMBER(9, 6) AS commission_rate,
    o.claims_on_order::INT AS claims_on_order,
    (
      o.claim_rank = 1
    )::BOOLEAN AS is_last_click,
    COALESCE(o.order_status IN ('CANCELLED', 'RETURNED'), FALSE)::BOOLEAN AS is_reversible,
    IFF(
      o.claim_rank = 1 AND COALESCE(o.order_status NOT IN ('CANCELLED', 'RETURNED'), FALSE),
      ROUND(o.commission * o.rate_to_usd, 2),
      0
    )::NUMBER(18, 2) AS payable_commission_usd
  FROM with_order AS o
)

SELECT *
FROM final
