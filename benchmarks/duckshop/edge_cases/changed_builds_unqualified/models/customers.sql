MODEL (description "Customers in the target schema, sharing a table name with the raw customers source.", materialized table);
SELECT customer_id FROM __ref("stg_customers")
