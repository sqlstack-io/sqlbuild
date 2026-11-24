MODEL (
  description "Order lines with one field read from the order payload.",
  materialized incremental,
  incremental_strategy merge,
  unique_key [order_id, line_no],
);

SELECT
  l.order_id,
  l.line_no,
  o.payload:customer::string AS customer_id
FROM __ref("order_lines") l
JOIN __ref("orders") o ON o.order_id = l.order_id
