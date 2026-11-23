MODEL (
  materialized table,
  tags [marts],
  description "Customer dimension with lifetime metrics.",
  columns (
    customer_id (nullable false, audits [not_null, unique]),
  ),
);

SELECT
  c.customer_id,
  c.first_name,
  c.last_name,
  c.email,
  c.created_at AS customer_since,
  COUNT(DISTINCT o.order_id) AS lifetime_orders,
  COALESCE(SUM(p.amount_cents), 0) AS lifetime_spend_cents
FROM __ref("stg_customers") c
LEFT JOIN __ref("stg_orders") o ON c.customer_id = o.customer_id
LEFT JOIN __ref("stg_payments") p ON o.order_id = p.order_id AND p.payment_status = 'success'
GROUP BY c.customer_id, c.first_name, c.last_name, c.email, c.created_at
