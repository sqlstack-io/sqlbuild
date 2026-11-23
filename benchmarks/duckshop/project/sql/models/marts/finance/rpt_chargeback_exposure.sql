MODEL (
  description """
    One row per reason family and outcome with chargeback counts, USD at risk and overdue responses
  """,
  columns (
    exposure_key (type VARCHAR, nullable false, audits [not_null, unique]),
    reason_family (type VARCHAR),
    outcome (type VARCHAR),
    chargebacks (type INT),
    amount_usd (type NUMBER(18, 2)),
    at_risk_usd (type NUMBER(18, 2)),
    overdue_responses (type INT),
    with_delivery_proof (type INT),
    repeat_customer_chargebacks (type INT),
  ),
);

WITH chargebacks AS (
  SELECT *
  FROM __ref("int_chargeback_exposure")
),

final AS (
  SELECT
    (
      c.reason_family || ':' || c.outcome
    )::VARCHAR AS exposure_key,
    c.reason_family,
    c.outcome,
    COUNT(*)::INT AS chargebacks,
    SUM(c.amount_usd)::NUMBER(18, 2) AS amount_usd,
    SUM(c.at_risk_usd)::NUMBER(18, 2) AS at_risk_usd,
    COUNT_IF(c.is_overdue)::INT AS overdue_responses,
    COUNT_IF(c.has_delivery_proof)::INT AS with_delivery_proof,
    COUNT_IF(c.prior_chargebacks_for_customer > 0)::INT AS repeat_customer_chargebacks
  FROM chargebacks AS c
  GROUP BY
    c.reason_family,
    c.outcome
)

SELECT *
FROM final
