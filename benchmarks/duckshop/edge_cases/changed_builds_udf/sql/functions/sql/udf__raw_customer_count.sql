FUNCTION (
  description "Number of customers from an id on, counted straight from the raw table without declaring it",
  arguments (p_customer_id INTEGER),
  returns BIGINT,
);

(SELECT count(*) FROM raw.customers AS c WHERE c.customer_id >= p_customer_id)
