MODEL (
  description "One row per line option with the whole parent line copied onto it.",
);

SELECT
  r.order_id,
  o.value AS line_json,
  l.value:code::string AS option_code
FROM __ref("orders") r, LATERAL FLATTEN(input => r.payload:lines) o, LATERAL FLATTEN(o.value:options) l
