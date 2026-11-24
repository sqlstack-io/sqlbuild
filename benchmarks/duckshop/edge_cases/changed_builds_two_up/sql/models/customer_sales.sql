MODEL (description "Sales per customer, two levels below changes held by different jobs.", materialized table);
SELECT customer_id, orders FROM __ref("customer_orders") WHERE orders > 0
