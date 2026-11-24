FUNCTION (
  description "Test function customer_orders.",
  arguments (p_customer_id INTEGER),
  returns table (
    order_id INTEGER,
    ordered_at TIMESTAMP,
    waffle_name VARCHAR,
    line_total_cents INTEGER,
    order_status VARCHAR,
    is_completed_order BOOLEAN
  )
);

SELECT
  order_id,
  ordered_at,
  waffle_name,
  line_total_cents,
  order_status,
  is_completed_order
FROM __ref("fact_orders")
WHERE customer_id = p_customer_id
