MODEL (description "Declares the raw customers source but also reads the customers model by its unqualified name.", materialized table);
SELECT r.customer_id, count(c.customer_id) AS modelled
FROM __source("raw_customers") AS r
LEFT JOIN customers AS c USING (customer_id)
GROUP BY r.customer_id
