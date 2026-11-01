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
    'VV-01:[''a$b'']' AS element_id,
    'VV-01' AS value_id,
    '[''a$b'']' AS element_path,
    'a$b' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-01:[''a-b'']' AS element_id,
    'VV-01' AS value_id,
    '[''a-b'']' AS element_path,
    'a-b' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'OBJECT' AS element_kind
  UNION ALL
  SELECT
    'VV-01:[''a-b''][''@x'']' AS element_id,
    'VV-01' AS value_id,
    '[''a-b''][''@x'']' AS element_path,
    '@x' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-01:[''a[b'']' AS element_id,
    'VV-01' AS value_id,
    '[''a[b'']' AS element_path,
    'a[b' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'ARRAY' AS element_kind
  UNION ALL
  SELECT
    'VV-01:[''a[b''][0]' AS element_id,
    'VV-01' AS value_id,
    '[''a[b''][0]' AS element_path,
    CAST(NULL AS VARCHAR) AS element_key,
    0 AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-01:[''a[b''][1]' AS element_id,
    'VV-01' AS value_id,
    '[''a[b''][1]' AS element_path,
    CAST(NULL AS VARCHAR) AS element_key,
    1 AS element_index,
    'OBJECT' AS element_kind
  UNION ALL
  SELECT
    'VV-01:[''a[b''][1].k' AS element_id,
    'VV-01' AS value_id,
    '[''a[b''][1].k' AS element_path,
    'k' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-02:[0]' AS element_id,
    'VV-02' AS value_id,
    '[0]' AS element_path,
    CAST(NULL AS VARCHAR) AS element_key,
    0 AS element_index,
    'OBJECT' AS element_kind
  UNION ALL
  SELECT
    'VV-02:[0][''x y'']' AS element_id,
    'VV-02' AS value_id,
    '[0][''x y'']' AS element_path,
    'x y' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-02:[1]' AS element_id,
    'VV-02' AS value_id,
    '[1]' AS element_path,
    CAST(NULL AS VARCHAR) AS element_key,
    1 AS element_index,
    'ARRAY' AS element_kind
  UNION ALL
  SELECT
    'VV-02:[1][0]' AS element_id,
    'VV-02' AS value_id,
    '[1][0]' AS element_path,
    CAST(NULL AS VARCHAR) AS element_key,
    0 AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-04:['''']' AS element_id,
    'VV-04' AS value_id,
    '['''']' AS element_path,
    '' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-04:[''9z'']' AS element_id,
    'VV-04' AS value_id,
    '[''9z'']' AS element_path,
    '9z' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-04:[''a.b'']' AS element_id,
    'VV-04' AS value_id,
    '[''a.b'']' AS element_path,
    'a.b' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-04:[''plain key'']' AS element_id,
    'VV-04' AS value_id,
    '[''plain key'']' AS element_path,
    'plain key' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
  UNION ALL
  SELECT
    'VV-04:é' AS element_id,
    'VV-04' AS value_id,
    'é' AS element_path,
    'é' AS element_key,
    CAST(NULL AS NUMBER(38, 0)) AS element_index,
    'INTEGER' AS element_kind
)
