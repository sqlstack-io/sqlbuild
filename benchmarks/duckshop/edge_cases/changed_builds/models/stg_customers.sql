MODEL (description "Staged customers.", materialized table);
SELECT customer_id FROM __source("raw_customers")
