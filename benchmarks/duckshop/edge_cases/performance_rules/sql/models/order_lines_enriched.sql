MODEL (
  description "Order lines with the whole order payload copied onto every line.",
  materialized incremental,
  incremental_strategy merge,
  unique_key [order_id, line_no],
);

SELECT
  l.order_id,
  l.line_no,
  l.sku,
  o.payload AS order_payload
FROM __ref("order_lines") l
JOIN __ref("orders") o ON o.order_id = l.order_id
