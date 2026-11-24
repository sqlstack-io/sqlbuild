MODEL (
  description """
    One row per chargeback with its amount in USD, the evidence document kinds submitted, the
    response deadline for its reason family, and the amount still at risk on the valuation date.
  """,
  constants (
    _response_days_by_family (fraud 20, consumer_dispute 30, processing 45),
  ),
  columns (
    chargeback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    payment_id (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    customer_id (type VARCHAR),
    payment_method (type VARCHAR),
    reason_code (type VARCHAR),
    reason_family (type VARCHAR),
    opened_at (type TIMESTAMP_TZ),
    response_due_on (type DATE),
    outcome (type VARCHAR),
    amount_usd (type NUMBER(18, 2)),
    share_of_payment (type NUMBER(9, 6)),
    evidence_kinds (type ARRAY),
    has_delivery_proof (type BOOLEAN),
    days_open (type INT),
    is_overdue (type BOOLEAN),
    at_risk_usd (type NUMBER(18, 2)),
    prior_chargebacks_for_customer (type INT),
  ),
);

WITH chargebacks AS (
  SELECT *
  FROM __ref("stg_chargebacks")
),

payments AS (
  SELECT *
  FROM __ref("stg_payments")
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

evidence AS (
  SELECT
    c.chargeback_id,
    ARRAY_AGG(DISTINCT LOWER(d.value:kind::VARCHAR)) WITHIN GROUP (
      ORDER BY LOWER(d.value:kind::VARCHAR)
    ) AS evidence_kinds,
    BOOLOR_AGG(REGEXP_LIKE(
      LOWER(d.value:kind::VARCHAR),
      '.*(tracking|delivery|signature|pod).*'
    )) AS has_delivery_proof
  FROM chargebacks AS c, LATERAL FLATTEN(input => c.evidence:documents) AS d
  GROUP BY
    c.chargeback_id
),

with_payment AS (
  SELECT
    c.chargeback_id,
    c.payment_id,
    p.order_id,
    p.payment_method,
    c.reason_code,
    c.reason_family,
    c.opened_at,
    CONVERT_TIMEZONE('UTC', c.opened_at)::DATE AS opened_date,
    c.closed_at,
    c.outcome,
    c.amount_minor,
    c.currency_code,
    p.amount_minor AS payment_amount_minor
  FROM chargebacks AS c
  LEFT JOIN payments AS p
    ON c.payment_id = p.payment_id
),

with_rate AS (
  SELECT
    w.chargeback_id,
    w.payment_id,
    w.order_id,
    w.payment_method,
    w.reason_code,
    w.reason_family,
    w.opened_at,
    w.opened_date,
    w.closed_at,
    w.outcome,
    w.amount_minor,
    w.payment_amount_minor,
    cur.minor_unit_exponent,
    fx.rate_to_usd
  FROM with_payment AS w
  LEFT JOIN currencies AS cur
    ON w.currency_code = cur.currency_code
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.opened_date >= fx.rate_date)
    ON w.currency_code = fx.currency_code
),

with_order AS (
  SELECT
    r.chargeback_id,
    r.payment_id,
    r.order_id,
    o.customer_id,
    r.payment_method,
    r.reason_code,
    r.reason_family,
    r.opened_at,
    r.opened_date,
    DATEADD(
      'day',
      COALESCE(@const("_response_days_by_family")[r.reason_family]::INT, 30),
      r.opened_date
    ) AS response_due_on,
    r.closed_at,
    r.outcome,
    ROUND(
      @sql.money_from_minor(r.amount_minor, COALESCE(r.minor_unit_exponent, 2), places := 6) * r.rate_to_usd,
      2
    ) AS amount_usd,
    r.amount_minor / NULLIF(r.payment_amount_minor, 0) AS share_of_payment
  FROM with_rate AS r
  LEFT JOIN orders AS o
    ON r.order_id = o.order_id
),

customer_history AS (
  SELECT
    w.chargeback_id,
    COUNT(*) OVER (
      PARTITION BY w.customer_id
      ORDER BY w.opened_at
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )
      - 1 AS prior_chargebacks_for_customer
  FROM with_order AS w
),

final AS (
  SELECT
    w.chargeback_id,
    w.payment_id,
    w.order_id,
    w.customer_id,
    w.payment_method,
    w.reason_code,
    w.reason_family,
    w.opened_at,
    w.response_due_on::DATE AS response_due_on,
    w.outcome,
    w.amount_usd::NUMBER(18, 2) AS amount_usd,
    ROUND(w.share_of_payment, 6)::NUMBER(9, 6) AS share_of_payment,
    COALESCE(e.evidence_kinds, ARRAY_CONSTRUCT())::ARRAY AS evidence_kinds,
    COALESCE(e.has_delivery_proof, FALSE)::BOOLEAN AS has_delivery_proof,
    DATEDIFF(
      'day',
      w.opened_date,
      COALESCE(CONVERT_TIMEZONE('UTC', w.closed_at)::DATE, '@@valuation_date'::DATE)
    )::INT AS days_open,
    (
      w.closed_at IS NULL
      AND w.response_due_on < '@@valuation_date'::DATE
    )::BOOLEAN AS is_overdue,
    IFF(w.outcome IN ('open', 'lost', 'pending'), w.amount_usd, 0)::NUMBER(18, 2) AS at_risk_usd,
    h.prior_chargebacks_for_customer::INT AS prior_chargebacks_for_customer
  FROM with_order AS w
  INNER JOIN customer_history AS h
    ON w.chargeback_id = h.chargeback_id
  LEFT JOIN evidence AS e
    ON w.chargeback_id = e.chargeback_id
)

SELECT *
FROM final
