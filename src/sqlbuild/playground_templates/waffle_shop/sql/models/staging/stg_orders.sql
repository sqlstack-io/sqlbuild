MODEL (
  materialized view,
  tags [staging],
  description "Cleaned order records.",
  columns (
    order_id (nullable false, audits [not_null, unique]),
    customer_id (nullable false, audits [not_null]),
    waffle_type_id (
      audits [relationships (to __seed("waffle_types"), field waffle_type_id)],
    ),
    status (
      audits [
        accepted_values (values ["placed", "preparing", "ready", "completed", "cancelled"]),
      ],
    ),
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
