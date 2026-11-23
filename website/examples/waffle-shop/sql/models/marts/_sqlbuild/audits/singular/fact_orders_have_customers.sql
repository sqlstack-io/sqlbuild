AUDIT (name "fact_orders_have_customers");

SELECT f.order_id, f.customer_id
FROM __ref("fact_orders") f
LEFT JOIN __ref("dim_customers") d ON f.customer_id = d.customer_id
WHERE d.customer_id IS NULL
