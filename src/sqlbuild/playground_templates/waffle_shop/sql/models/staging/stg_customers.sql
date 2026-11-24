MODEL (
  materialized view,
  tags [staging],
  description "Cleaned customer records.",
  columns (
    customer_id (audits [not_null, unique]),
    email (audits [not_null]),
  ),
);

SELECT
  id AS customer_id,
  first_name,
  last_name,
  email,
  created_at
FROM __source("raw__customers")
