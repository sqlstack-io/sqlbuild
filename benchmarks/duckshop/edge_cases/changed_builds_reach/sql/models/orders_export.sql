MODEL (description "Orders exported by a job that nothing triggers.", materialized table);
SELECT order_id FROM __ref("stg_orders")
