MODEL (description "Ranks per order", materialized table);

SELECT order_id FROM __ref("order_totals")
