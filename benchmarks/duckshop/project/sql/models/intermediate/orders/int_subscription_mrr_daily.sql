MODEL (
  description """
    One row per subscription and day it was active, with its monthly recurring revenue in USD
    normalised from its billing interval.
  """,
  columns (
    subscription_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    subscription_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    calendar_date (type DATE, audits [not_null]),
    mrr_usd (type NUMBER(18, 4)),
    is_first_day (type BOOLEAN),
  ),
);

WITH periods AS (
  SELECT *
  FROM __ref("int_subscription_periods")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

@sql.date_spine(@const("history_start_date"), 730)

active_periods AS (
  SELECT
    p.subscription_id,
    p.customer_id,
    p.period_start::DATE AS start_date,
    COALESCE(p.period_end::DATE, '2026-12-31'::DATE) AS end_date,
    p.billing_amount_minor,
    p.billing_currency_code,
    p.interval_days
  FROM periods AS p
  WHERE p.period_state = 'active'
), active_days AS (
  SELECT
    a.subscription_id,
    a.customer_id,
    d.calendar_date,
    a.billing_amount_minor,
    a.billing_currency_code,
    a.interval_days
  FROM active_periods AS a
  INNER JOIN date_spine AS d
    ON d.calendar_date >= a.start_date
    AND d.calendar_date < a.end_date
), with_exponent AS (
  SELECT
    ad.subscription_id,
    ad.customer_id,
    ad.calendar_date,
    ad.billing_amount_minor,
    ad.billing_currency_code,
    ad.interval_days,
    c.minor_unit_exponent
  FROM active_days AS ad
  INNER JOIN currencies AS c
    ON ad.billing_currency_code = c.currency_code
), with_rate AS (
  SELECT
    w.subscription_id,
    w.customer_id,
    w.calendar_date,
    w.billing_amount_minor,
    w.interval_days,
    w.minor_unit_exponent,
    fx.rate_to_usd
  FROM with_exponent AS w
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.calendar_date >= fx.rate_date)
    ON w.billing_currency_code = fx.currency_code
), final AS (
  SELECT
    (
      r.subscription_id || ':' || TO_CHAR(r.calendar_date, 'YYYYMMDD')
    )::VARCHAR AS subscription_day_key,
    r.subscription_id,
    r.customer_id,
    r.calendar_date,
    ROUND(
      @sql.money_from_minor(r.billing_amount_minor, r.minor_unit_exponent, places := 6) * r.rate_to_usd * 30.4375
        / NULLIF(r.interval_days, 0),
      4
    )::NUMBER(18, 4) AS mrr_usd,
    (
      r.calendar_date = MIN(r.calendar_date) OVER (PARTITION BY r.subscription_id)
    ) AS is_first_day
  FROM with_rate AS r
)
SELECT *
FROM final
