MODEL (
  description "One row per order with its customer and amount",
  materialized table,
  columns (
    order_id (nullable false, audits [not_null, unique]),
    customer_id (nullable false, audits [not_null]),
    amount_cents (nullable false, audits [not_null]),
  ),
);

SELECT
  order_id,
  customer_id,
  amount_cents
FROM __source("raw_orders")
