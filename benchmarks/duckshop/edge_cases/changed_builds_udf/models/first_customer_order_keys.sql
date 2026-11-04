MODEL (description "Order keys of the first customer, read only through a table function.", materialized table);
SELECT k.order_key
FROM __table_fn("table_fn__customer_order_keys")(1) AS k
