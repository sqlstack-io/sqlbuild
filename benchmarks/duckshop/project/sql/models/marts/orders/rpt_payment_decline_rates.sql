MODEL (
  description "One row per month, payment method and decline family with attempt counts and rates",
  columns (
    decline_key (type VARCHAR, nullable false, audits [not_null, unique]),
    attempt_month (type DATE),
    payment_method (type VARCHAR),
    decline_family (type VARCHAR),
    attempts (type INT),
    share_of_method_attempts (type NUMBER(9, 4)),
    attempted_usd (type NUMBER(18, 2)),
    recovered_on_retry (type INT),
    retry_recovery_rate (type NUMBER(9, 4)),
  ),
);

WITH payments AS (
  SELECT *
  FROM __ref("fct_payments")
),

monthly AS (
  SELECT
    DATE_TRUNC(
      'month',
      CONVERT_TIMEZONE('UTC', p.authorised_at)
    )::DATE AS attempt_month,
    p.payment_method,
    p.decline_family,
    COUNT(*) AS attempts,
    SUM(p.amount_usd) AS attempted_usd,
    COUNT_IF(NOT p.is_successful AND NOT p.is_final_attempt) AS recovered_on_retry
  FROM payments AS p
  GROUP BY
    1,
    2,
    3
),

final AS (
  SELECT
    (
      TO_VARCHAR(m.attempt_month, 'YYYY-MM')
        || ':'
        || COALESCE(m.payment_method, '?')
        || ':'
        || m.decline_family
    )::VARCHAR AS decline_key,
    m.attempt_month,
    m.payment_method,
    m.decline_family,
    m.attempts::INT AS attempts,
    ROUND(
      RATIO_TO_REPORT(m.attempts) OVER (PARTITION BY m.attempt_month, m.payment_method),
      4
    )::NUMBER(9, 4) AS share_of_method_attempts,
    m.attempted_usd::NUMBER(18, 2) AS attempted_usd,
    m.recovered_on_retry::INT AS recovered_on_retry,
    @py.round_half_even(@sql.safe_divide(m.recovered_on_retry, m.attempts), 4)::NUMBER(9, 4) AS retry_recovery_rate
  FROM monthly AS m
)

SELECT *
FROM final
