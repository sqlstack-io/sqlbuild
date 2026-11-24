FUNCTION (
  description "Orders placed by one customer with their USD value, status and sequence number",
  arguments (p_customer_id VARCHAR),
  returns table (
    order_id VARCHAR,
    placed_at TIMESTAMP_TZ,
    order_status VARCHAR,
    amount_usd "NUMBER(18, 2)",
    order_number INT,
    is_first_order BOOLEAN
  )
);

WITH orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

final AS (
  SELECT
    o.order_id,
    o.placed_at,
    o.order_status,
    o.amount_usd,
    o.order_number,
    o.is_first_order
  FROM orders AS o
  WHERE o.customer_id = p_customer_id
)

SELECT *
FROM final
