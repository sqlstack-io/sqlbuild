MODEL (description "Staged orders.", materialized table);
SELECT order_id, customer_id FROM __source("raw_orders")
