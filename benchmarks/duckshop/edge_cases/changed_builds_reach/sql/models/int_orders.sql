MODEL (description "Orders with their customer, refreshed by the staging job.", materialized table);
SELECT order_id, customer_id FROM __ref("stg_orders")
