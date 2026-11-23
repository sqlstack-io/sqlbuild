MODEL (
  description "One row per order line.",
  materialized incremental,
  incremental_strategy merge,
  unique_key [order_id, line_no],
);

SELECT
  o.order_id,
  l.index AS line_no,
  l.value:sku::string AS sku
FROM __ref("orders") o, LATERAL FLATTEN(input => o.payload:lines) l
