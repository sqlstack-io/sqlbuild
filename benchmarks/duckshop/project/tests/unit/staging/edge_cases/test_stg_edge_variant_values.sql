TEST (name "stg_edge_variant_values__turns_missing_values_into_json_null");

WITH __source__edge__variant_values AS (
  SELECT
    v.value_id,
    v.group_code,
    v.sort_key::NUMBER(38, 0) AS sort_key,
    PARSE_JSON(v.attribute_value) AS attribute_value,
    v.value_text,
    '2026-03-06 00:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('VV-01', 'V-1', 1, '{"b": 2, "a": 1}', '{"b": 2, "a": 1}'),
    ('VV-02', 'V-1', 2, 'null', 'null'),
    ('VV-03', 'V-1', 3, NULL, NULL) AS v(value_id, group_code, sort_key, attribute_value, value_text)
),

__expected__stg_edge_variant_values AS (
  SELECT
    'VV-01' AS value_id,
    PARSE_JSON('{"a":1,"b":2}') AS attribute_value
  UNION ALL
  SELECT
    'VV-02' AS value_id,
    PARSE_JSON('null') AS attribute_value
  UNION ALL
  SELECT
    'VV-03' AS value_id,
    PARSE_JSON('null') AS attribute_value
)
