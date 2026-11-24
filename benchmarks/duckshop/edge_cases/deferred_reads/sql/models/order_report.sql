MODEL (description "A report a deferring build selects alone, reading the aliased orders.", materialized table);

SELECT order_id FROM __ref("orders")
