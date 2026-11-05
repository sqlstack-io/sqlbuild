MODEL (description "Orders per customer, two models downstream of both sources.", materialized table);
SELECT c.customer_id, count(o.order_id) AS orders
FROM __ref("stg_customers") AS c
LEFT JOIN __ref("stg_orders") AS o USING (customer_id)
GROUP BY c.customer_id
