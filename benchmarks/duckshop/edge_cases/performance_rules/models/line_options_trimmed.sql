MODEL (
  description "One row per line option with the parent line minus its options.",
);

SELECT
  r.order_id,
  OBJECT_DELETE(o.value, 'options') AS line_json,
  l.value:code::string AS option_code
FROM __ref("orders") r, LATERAL FLATTEN(input => r.payload:lines) o, LATERAL FLATTEN(o.value:options) l
