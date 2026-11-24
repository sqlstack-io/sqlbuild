FUNCTION (
  description "Order keys of one customer, read from staged orders",
  arguments (p_customer_id INTEGER),
  returns table (order_key INTEGER, customer_id INTEGER)
);

SELECT o.order_key, o.customer_id
FROM __ref("stg_orders") AS o
WHERE o.customer_id = p_customer_id
