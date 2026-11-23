MODEL (
  description "One row per refund with its amount in major units and USD, its reason and timing",
  contract enforced,
  model_schema money,
  audit_factories [fact_table_quality],
  columns (
    refund_id (type VARCHAR, nullable false, audits [not_null, unique]),
    payment_id (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    refunded_at (type TIMESTAMP_TZ),
    refunded_date (type DATE),
    reason_category (type VARCHAR),
    days_after_payment (type INT),
    is_full_refund (type BOOLEAN),
  ),
);

WITH refunds AS (
  SELECT *
  FROM __ref("int_refunds_usd")
),

final AS (
  SELECT
    r.refund_id,
    r.payment_id,
    r.order_id,
    r.currency_code,
    r.amount,
    r.amount_usd,
    r.refunded_at,
    r.refunded_date,
    r.reason_category,
    r.days_after_payment,
    r.is_full_refund
  FROM refunds AS r
)

SELECT *
FROM final
