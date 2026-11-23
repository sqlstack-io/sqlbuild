MODEL (description "One row per waffle order with its customer and line total", materialized table);

SELECT
  o.order_id,
  o.customer_id,
  c.plan_name,
  o.waffle_type,
  o.quantity,
  o.price_cents,
  o.quantity * o.price_cents AS revenue_cents,
  o.load_seq
FROM __source("raw_orders") o
LEFT JOIN __source("raw_customers") c ON o.customer_id = c.customer_id
