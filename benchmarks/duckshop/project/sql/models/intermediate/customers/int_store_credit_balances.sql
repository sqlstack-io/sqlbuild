MODEL (
  description """
    One row per customer and currency with the store credit balance, the amount expiring within 30
    days and its age
  """,
  columns (
    store_credit_key (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    balance_minor (type INT),
    expiring_30d_minor (type INT),
    first_posted_at (type TIMESTAMP_TZ),
    last_posted_at (type TIMESTAMP_TZ),
    posting_count (type INT),
    goodwill_share (type NUMBER(9, 6)),
    balance_usd (type NUMBER(18, 2)),
  ),
);

WITH ledger AS (
  SELECT *
  FROM __ref("stg_store_credit_ledger")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

grouped AS (
  SELECT
    l.customer_id,
    l.currency_code,
    SUM(l.amount_minor) AS balance_minor,
    SUM(
      IFF(
        l.amount_minor > 0
        AND l.expires_on BETWEEN '@@valuation_date'::DATE AND DATEADD(
          'day',
          30,
          '@@valuation_date'::DATE
        ),
        l.amount_minor,
        0
      )
    ) AS expiring_30d_minor,
    MIN(l.posted_at) AS first_posted_at,
    MAX(l.posted_at) AS last_posted_at,
    COUNT(*) AS posting_count,
    SUM(IFF(l.reason_code LIKE 'GOODWILL%', l.amount_minor, 0)) AS goodwill_minor,
    SUM(IFF(l.amount_minor > 0, l.amount_minor, 0)) AS issued_minor
  FROM ledger AS l
  GROUP BY
    l.customer_id,
    l.currency_code
),

valued AS (
  SELECT
    g.customer_id,
    g.currency_code,
    g.balance_minor,
    g.expiring_30d_minor,
    g.first_posted_at,
    g.last_posted_at,
    g.posting_count,
    g.goodwill_minor,
    g.issued_minor,
    '@@valuation_date'::DATE AS valuation_date,
    c.minor_unit_exponent
  FROM grouped AS g
  LEFT JOIN currencies AS c
    ON g.currency_code = c.currency_code
),

with_rate AS (
  SELECT
    v.customer_id,
    v.currency_code,
    v.balance_minor,
    v.expiring_30d_minor,
    v.first_posted_at,
    v.last_posted_at,
    v.posting_count,
    v.goodwill_minor,
    v.issued_minor,
    v.minor_unit_exponent,
    fx.rate_to_usd
  FROM valued AS v
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (v.valuation_date >= fx.rate_date)
    ON v.currency_code = fx.currency_code
),

final AS (
  SELECT
    (
      g.customer_id || ':' || g.currency_code
    )::VARCHAR AS store_credit_key,
    g.customer_id,
    g.currency_code,
    g.balance_minor::INT AS balance_minor,
    LEAST(g.expiring_30d_minor, GREATEST(g.balance_minor, 0))::INT AS expiring_30d_minor,
    g.first_posted_at,
    g.last_posted_at,
    g.posting_count::INT AS posting_count,
    ROUND(g.goodwill_minor / NULLIF(g.issued_minor, 0), 6)::NUMBER(9, 6) AS goodwill_share,
    ROUND(
      @sql.money_from_minor(g.balance_minor, COALESCE(g.minor_unit_exponent, 2), places := 6) * g.rate_to_usd,
      2
    )::NUMBER(18, 2) AS balance_usd
  FROM with_rate AS g
)

SELECT *
FROM final
