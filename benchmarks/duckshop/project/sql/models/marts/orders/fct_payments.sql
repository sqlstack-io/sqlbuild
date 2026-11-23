MODEL (
  description "One row per payment attempt with its amount in major units and USD and its outcome",
  contract enforced,
  model_schema money,
  audit_factories [fact_table_quality],
  columns (
    payment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    attempt_number (type INT),
    is_final_attempt (type BOOLEAN),
    payment_method (type VARCHAR),
    authorised_at (type TIMESTAMP_TZ),
    is_successful (type BOOLEAN),
    decline_family (type VARCHAR),
    risk_band (type VARCHAR),
    card_scheme (type VARCHAR),
    had_chargeback (type BOOLEAN),
  ),
);

WITH attempts AS (
  SELECT *
  FROM __ref("int_payment_attempts")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

with_exponent AS (
  SELECT
    a.payment_id,
    a.order_id,
    a.attempt_number,
    a.is_final_attempt,
    a.payment_method,
    a.authorised_at,
    CONVERT_TIMEZONE('UTC', a.authorised_at)::DATE AS authorised_date,
    a.is_successful,
    a.decline_family,
    a.risk_band,
    a.card_scheme,
    a.had_chargeback,
    a.currency_code,
    a.amount_minor,
    c.minor_unit_exponent
  FROM attempts AS a
  INNER JOIN currencies AS c
    ON a.currency_code = c.currency_code
),

with_rate AS (
  SELECT
    w.payment_id,
    w.order_id,
    w.attempt_number,
    w.is_final_attempt,
    w.payment_method,
    w.authorised_at,
    w.is_successful,
    w.decline_family,
    w.risk_band,
    w.card_scheme,
    w.had_chargeback,
    w.currency_code,
    w.amount_minor,
    w.minor_unit_exponent,
    fx.rate_to_usd
  FROM with_exponent AS w
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.authorised_date >= fx.rate_date)
    ON w.currency_code = fx.currency_code
),

final AS (
  SELECT
    r.payment_id,
    r.order_id,
    r.attempt_number,
    r.is_final_attempt,
    r.payment_method,
    r.authorised_at,
    r.is_successful,
    r.decline_family,
    r.risk_band,
    r.card_scheme,
    r.had_chargeback,
    r.currency_code,
    @sql.money_columns(r)
  FROM with_rate AS r
)

SELECT *
FROM final
