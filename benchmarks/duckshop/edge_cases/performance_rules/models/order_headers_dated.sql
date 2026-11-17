MODEL (
  description "One row per order: keyed by order number and date for pruning, but order_id alone is unique.",
  materialized incremental,
  incremental_strategy merge,
  unique_key [order_number, order_date],
  columns (
    order_id (type NUMBER, nullable false, audits [unique]),
  ),
);

SELECT
  h.order_id,
  h.order_number,
  CURRENT_DATE() AS order_date,
  h.header AS order_header
FROM __ref("order_headers") h
