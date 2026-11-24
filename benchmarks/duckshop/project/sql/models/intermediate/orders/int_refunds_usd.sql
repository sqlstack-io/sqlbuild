MODEL (
  description """
    One row per refund in major units and USD at the reference rate on or before the refund date
  """,
  audits [
    match_rate (
      name "fx_rate_found",
      matched "amount_usd IS NOT NULL",
      minimum_samples 50,
      evidence_limit 20,
      thresholds (warn (below 99.5), error (below 95)),
    ),
  ],
  columns (
    refund_id (type VARCHAR, nullable false, audits [not_null, unique]),
    payment_id (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    currency_code (type VARCHAR),
    refunded_at (type TIMESTAMP_TZ),
    refunded_date (type DATE),
    reason_category (type VARCHAR),
    amount (type NUMBER(38, 6)),
    amount_usd (type NUMBER(18, 2)),
    days_after_payment (type INT),
    is_full_refund (type BOOLEAN),
  ),
);

WITH refunds AS (
  SELECT *
  FROM __ref("stg_refunds")
),

payments AS (
  SELECT *
  FROM __ref("stg_payments")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

refund_context AS (
  SELECT
    r.refund_id,
    r.payment_id,
    p.order_id,
    p.currency_code,
    r.refunded_at,
    CONVERT_TIMEZONE('UTC', r.refunded_at)::DATE AS refunded_date,
    r.reason_category,
    r.amount_minor,
    p.amount_minor AS payment_amount_minor,
    p.captured_at
  FROM refunds AS r
  INNER JOIN payments AS p
    ON r.payment_id = p.payment_id
),

with_exponent AS (
  SELECT
    rc.refund_id,
    rc.payment_id,
    rc.order_id,
    rc.currency_code,
    rc.refunded_at,
    rc.refunded_date,
    rc.reason_category,
    rc.amount_minor,
    rc.payment_amount_minor,
    rc.captured_at,
    c.minor_unit_exponent
  FROM refund_context AS rc
  INNER JOIN currencies AS c
    ON rc.currency_code = c.currency_code
),

with_rate AS (
  SELECT
    w.refund_id,
    w.payment_id,
    w.order_id,
    w.currency_code,
    w.refunded_at,
    w.refunded_date,
    w.reason_category,
    w.amount_minor,
    w.payment_amount_minor,
    w.captured_at,
    w.minor_unit_exponent,
    fx.rate_to_usd
  FROM with_exponent AS w
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.refunded_date >= fx.rate_date)
    ON w.currency_code = fx.currency_code
),

final AS (
  SELECT
    r.refund_id,
    r.payment_id,
    r.order_id,
    r.currency_code,
    r.refunded_at,
    r.refunded_date,
    r.reason_category,
    @sql.money_columns(r),
    DATEDIFF('day', r.captured_at, r.refunded_at)::INT AS days_after_payment,
    (
      r.amount_minor >= r.payment_amount_minor
    ) AS is_full_refund
  FROM with_rate AS r
)

SELECT *
FROM final
