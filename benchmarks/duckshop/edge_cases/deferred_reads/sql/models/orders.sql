MODEL (description "Orders stored under an alias, so the deferred target lists them as orders_v2.", materialized table, alias "orders_v2");

SELECT 1 AS order_id
