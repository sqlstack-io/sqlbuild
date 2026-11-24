MODEL (description "Revenue per customer from completed waffle orders", materialized table);

SELECT
  customer_id,
  COALESCE(plan_name, 'unknown') AS plan_name,
  SUM(revenue_cents) AS revenue_cents,
  COUNT(*) AS order_count
FROM __ref("fact_waffle_orders")
GROUP BY customer_id, COALESCE(plan_name, 'unknown')
