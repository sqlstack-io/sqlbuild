MODEL (
  description "The published view of order lines.",
  materialized view,
);

SELECT order_id, line_no, order_payload
FROM __ref("shop__mart__order_lines")
