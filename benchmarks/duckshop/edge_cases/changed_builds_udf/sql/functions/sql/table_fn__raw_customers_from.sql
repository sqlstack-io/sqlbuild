FUNCTION (
  description "Customers from an id on, read straight from the raw table without declaring it",
  arguments (p_customer_id INTEGER),
  returns table (customer_id INTEGER)
);

SELECT c.customer_id
FROM raw.customers AS c
WHERE c.customer_id >= p_customer_id
