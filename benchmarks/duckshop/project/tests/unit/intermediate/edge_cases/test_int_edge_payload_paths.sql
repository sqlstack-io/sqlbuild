TEST (name "int_edge_payload_paths__writes_recursive_paths_for_unusual_keys");

WITH __ref__stg_edge_variant_values AS (
  SELECT
    v.value_id,
    v.group_code,
    v.sort_key::NUMBER(38, 0) AS sort_key,
    IFF(
      v.attribute_value = '@date',
      TO_VARIANT('2026-03-01'::DATE),
      PARSE_JSON(v.attribute_value)
    ) AS attribute_value,
    v.value_text
  FROM VALUES
    ('VV-01', 'V-1', 1, '{"a$b": 1, "a-b": {"@x": 2}, "a[b": [3, {"k": 4}]}', NULL),
    ('VV-02', 'V-1', 2, '[{"x y": 1}, [2]]', NULL),
    ('VV-03', 'V-1', 3, '5', NULL),
    ('VV-04', 'V-1', 4, '{"\u00e9": 5, "plain key": 6, "": 7, "a.b": 8, "9z": 9}', NULL) AS v(value_id, group_code, sort_key, attribute_value, value_text)
),

__expected__int_edge_payload_paths AS (
  SELECT
    NULL AS element_id,
    NULL AS value_id,
    NULL AS element_path,
    NULL AS element_key,
    NULL AS element_index,
    NULL AS element_kind
)
