MODEL (
  description """
    One row per payment attempt with its position among the order's attempts, the time since the
    previous attempt, a normalised decline reason and card checks.
  """,
  enums (
    _decline_family [NONE, INSUFFICIENT_FUNDS, SUSPECTED_FRAUD, AUTHENTICATION, TECHNICAL, OTHER],
  ),
  columns (
    payment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    attempt_number (type INT, audits [not_null]),
    attempt_count (type INT),
    is_final_attempt (type BOOLEAN),
    payment_method (type VARCHAR),
    amount_minor (type INT),
    currency_code (type VARCHAR),
    authorised_at (type TIMESTAMP_TZ),
    seconds_since_previous_attempt (type INT),
    gateway_status (type VARCHAR),
    is_successful (type BOOLEAN),
    decline_family (type VARCHAR),
    gateway_error_code (type VARCHAR),
    risk_score (type NUMBER(5, 2)),
    risk_band (type VARCHAR),
    three_ds_outcome (type VARCHAR),
    card_bin_valid (type BOOLEAN),
    card_scheme (type VARCHAR),
    had_chargeback (type BOOLEAN),
  ),
);

WITH payments AS (
  SELECT *
  FROM __ref("stg_payments")
),

chargebacks AS (
  SELECT *
  FROM __ref("stg_chargebacks")
),

sequenced AS (
  SELECT
    p.payment_id,
    p.order_id,
    p.payment_method,
    p.amount_minor,
    p.currency_code,
    p.authorised_at,
    p.gateway_status,
    p.gateway_error_code,
    p.gateway_response,
    p.risk_score,
    p.three_ds_outcome,
    p.card_bin,
    ROW_NUMBER() OVER (
      PARTITION BY p.order_id
      ORDER BY p.authorised_at, p.payment_id
    ) AS attempt_number,
    COUNT(*) OVER (PARTITION BY p.order_id) AS attempt_count,
    LAG(p.authorised_at) OVER (
      PARTITION BY p.order_id
      ORDER BY p.authorised_at, p.payment_id
    ) AS previous_authorised_at
  FROM payments AS p
),

decline_reasons AS (
  SELECT
    s.payment_id,
    LOWER(COALESCE(s.gateway_response:status.message::VARCHAR, '')) AS decline_message
  FROM sequenced AS s
  WHERE s.gateway_status NOT IN ('captured', 'settled', 'authorised')
),

classified AS (
  SELECT
    d.payment_id,
    CASE
      WHEN REGEXP_LIKE(d.decline_message, '.*(insufficient|nsf|limit exceeded).*')
      THEN @enum("_decline_family").INSUFFICIENT_FUNDS
      WHEN REGEXP_LIKE(d.decline_message, '.*(fraud|stolen|lost card|pickup).*')
      THEN @enum("_decline_family").SUSPECTED_FRAUD
      WHEN REGEXP_LIKE(d.decline_message, '.*(3ds|authentication|challenge).*')
      THEN @enum("_decline_family").AUTHENTICATION
      WHEN REGEXP_LIKE(d.decline_message, '.*(timeout|unavailable|error [0-9]{3}).*')
      THEN @enum("_decline_family").TECHNICAL
      ELSE @enum("_decline_family").OTHER
    END AS decline_family
  FROM decline_reasons AS d
),

chargeback_flags AS (
  SELECT
    c.payment_id,
    COUNT(*) AS chargeback_count
  FROM chargebacks AS c
  GROUP BY
    c.payment_id
),

final AS (
  SELECT
    s.payment_id,
    s.order_id,
    s.attempt_number::INT AS attempt_number,
    s.attempt_count::INT AS attempt_count,
    (
      s.attempt_number = s.attempt_count
    ) AS is_final_attempt,
    s.payment_method,
    s.amount_minor::INT AS amount_minor,
    s.currency_code,
    s.authorised_at,
    DATEDIFF(
      'second',
      s.previous_authorised_at,
      s.authorised_at
    )::INT AS seconds_since_previous_attempt,
    s.gateway_status,
    (
      s.gateway_status IN ('captured', 'settled', 'authorised')
    ) AS is_successful,
    COALESCE(c.decline_family, @enum("_decline_family").NONE)::VARCHAR AS decline_family,
    s.gateway_error_code,
    s.risk_score,
    CASE
      WHEN s.risk_score IS NULL
      THEN 'unscored'
      WHEN s.risk_score >= 75
      THEN 'high'
      WHEN s.risk_score >= 40
      THEN 'medium'
      ELSE 'low'
    END::VARCHAR AS risk_band,
    s.three_ds_outcome,
    __udf("luhn_valid")(s.card_bin)::BOOLEAN AS card_bin_valid,
    CASE
      WHEN s.card_bin RLIKE '^4[0-9]*'
      THEN 'network_a'
      WHEN s.card_bin RLIKE '^(5[1-5]|2[2-7])[0-9]*'
      THEN 'network_b'
      WHEN s.card_bin RLIKE '^3[47][0-9]*'
      THEN 'network_c'
      WHEN s.card_bin IS NULL
      THEN NULL
      ELSE 'other'
    END::VARCHAR AS card_scheme,
    (
      COALESCE(cb.chargeback_count, 0) > 0
    ) AS had_chargeback
  FROM sequenced AS s
  LEFT JOIN classified AS c
    ON s.payment_id = c.payment_id
  LEFT JOIN chargeback_flags AS cb
    ON s.payment_id = cb.payment_id
)

SELECT *
FROM final
