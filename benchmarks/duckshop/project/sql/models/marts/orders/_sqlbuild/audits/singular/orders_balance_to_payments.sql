AUDIT (
  name "orders_balance_to_payments",
  severity warn,
);

WITH orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

payments AS (
  SELECT *
  FROM __ref("fct_payments")
),

successful_payments AS (
  SELECT
    p.order_id,
    SUM(p.amount) AS paid_amount
  FROM payments AS p
  WHERE p.is_successful
  GROUP BY
    p.order_id
),

final AS (
  SELECT
    o.order_id,
    o.amount,
    s.paid_amount
  FROM orders AS o
  LEFT JOIN successful_payments AS s
    ON o.order_id = s.order_id
  WHERE
    o.order_status IN ('PAID', 'PACKED', 'SHIPPED', 'DELIVERED')
    AND ABS(o.amount - COALESCE(s.paid_amount, 0)) > 0.01
)

SELECT *
FROM final
