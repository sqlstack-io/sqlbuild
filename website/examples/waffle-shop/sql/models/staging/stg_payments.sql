MODEL (
  materialized view,
  tags [staging],
  description "Cleaned payment records.",
  columns (
    payment_id (audits [not_null, unique]),
    order_id (
      audits [not_null, relationships (to __source("raw__orders"), field id)],
    ),
  ),
);

SELECT
  id AS payment_id,
  order_id,
  amount_cents,
  payment_method,
  paid_at,
  status AS payment_status
FROM __source("raw__payments")
