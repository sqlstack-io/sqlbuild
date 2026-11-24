MODEL (
  description "One row per order: the merge key adds a date for pruning, but order_id alone is unique.",
  materialized incremental,
  incremental_strategy merge,
  unique_key [order_id, order_date],
  columns (
    order_id (type NUMBER, nullable false, audits [unique]),
  ),
);

SELECT
  o.order_id,
  CURRENT_DATE() AS order_date,
  o.payload AS order_payload
FROM __ref("orders") o
