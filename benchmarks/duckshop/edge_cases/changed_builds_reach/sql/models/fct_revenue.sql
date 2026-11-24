MODEL (description "Order counts per customer, refreshed by the finance job.", materialized table);
SELECT customer_id, count(*) AS revenue FROM __ref("int_orders") GROUP BY customer_id
