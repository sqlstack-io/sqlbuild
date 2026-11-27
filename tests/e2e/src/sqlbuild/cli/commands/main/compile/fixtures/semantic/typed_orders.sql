MODEL (
  description "Test model typed_orders.",
  materialized view,
  contract enforced,
  columns (
    order_id (type BIGINT),
    customer_id (type BIGINT),
    waffle_type_id (type BIGINT),
    quantity (type BIGINT),
    ordered_at (type TIMESTAMP),
    status (type VARCHAR),
  ),
);

SELECT
  id AS order_id,
  customer_id,
  waffle_type_id,
  quantity,
  ordered_at,
  status
FROM __source("raw__orders")
