MODEL (description "Staged orders with a display key from a SQL function.", materialized table);
SELECT order_id, customer_id, __udf("udf__order_key")(order_id) AS order_key
FROM __source("raw_orders")
