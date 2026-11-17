MODEL (description "Orders read downstream of the cloned table.", materialized table);

SELECT order_id FROM __ref("fact_orders")
