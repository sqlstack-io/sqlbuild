MODEL (description "Joins a branch the change reaches now with one that waits for the finance job.", materialized table);
SELECT c.customer_id, count(i.order_id) AS orders, max(f.revenue) AS revenue
FROM __ref("stg_customers") AS c
LEFT JOIN __ref("int_orders") AS i USING (customer_id)
LEFT JOIN __ref("fct_revenue") AS f USING (customer_id)
GROUP BY c.customer_id
