MODEL (
  description "One row per line option with the parent line minus its options, and a hash of the whole line.",
);

SELECT
  r.order_id,
  OBJECT_DELETE(o.value, 'options') AS line_json,
  MD5(TO_VARCHAR(o.value)) AS line_hash,
  l.value:code::string AS option_code
FROM __ref("orders") r, LATERAL FLATTEN(input => r.payload:lines) o, LATERAL FLATTEN(o.value:options) l
