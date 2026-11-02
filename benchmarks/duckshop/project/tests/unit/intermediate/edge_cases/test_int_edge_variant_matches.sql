TEST (name "int_edge_variant_matches__compares_variants_by_type_and_value");

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
    ('VV-01', 'V-1', 1, '1', 'one'),
    ('VV-02', 'V-1', 2, '"1"', 'one as text'),
    ('VV-03', 'V-1', 3, '1.0', 'one with a fraction'),
    ('VV-04', 'V-1', 4, 'null', 'json null'),
    ('VV-05', 'V-2', 1, '{"a": 1, "b": 2}', 'keys a, b'),
    ('VV-06', 'V-2', 2, '{"b": 2, "a": 1}', 'keys b, a'),
    ('VV-07', 'V-2', 3, '"2026-03-01"', 'date text'),
    ('VV-08', 'V-2', 4, '"2026-03-01 10:15:00"', 'timestamp text'),
    ('VV-09', 'V-3', 1, '[1, 2]', 'array'),
    ('VV-10', 'V-3', 2, '[2, 1]', 'reversed array'),
    ('VV-11', 'V-4', 1, '@date', 'stored date'),
    ('VV-12', 'V-4', 2, '"2026-03-01"', 'date text') AS v(value_id, group_code, sort_key, attribute_value, value_text)
),

__expected__int_edge_variant_matches AS (
  SELECT
    'VV-01~VV-02' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":1,"VALUE_ID":"VV-01"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-01~VV-02-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":1,"VALUE_ID":"VV-01"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-01~VV-02-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":1,"VALUE_ID":"VV-01"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-01~VV-03' AS pair_id,
    TRUE AS values_equal,
    TRUE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":1,"VALUE_ID":"VV-01"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-01~VV-04' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":1,"VALUE_ID":"VV-01"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-02-DATE~VV-02-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1970-01-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"1970-01-01","GROUP_CODE":"V-1","SORT_KEY":102,"VALUE_ID":"VV-02-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-02~VV-02-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"1","GROUP_CODE":"V-1","SORT_KEY":2,"VALUE_ID":"VV-02"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-02~VV-02-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"1","GROUP_CODE":"V-1","SORT_KEY":2,"VALUE_ID":"VV-02"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-02~VV-03' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"1","GROUP_CODE":"V-1","SORT_KEY":2,"VALUE_ID":"VV-02"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-02~VV-04' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"1","GROUP_CODE":"V-1","SORT_KEY":2,"VALUE_ID":"VV-02"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-03~VV-02-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":3,"VALUE_ID":"VV-03"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-03~VV-02-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":3,"VALUE_ID":"VV-03"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-03~VV-04' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    '1' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":1,"GROUP_CODE":"V-1","SORT_KEY":3,"VALUE_ID":"VV-03"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-04~VV-02-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":null,"GROUP_CODE":"V-1","SORT_KEY":4,"VALUE_ID":"VV-04"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-04~VV-02-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'INTEGER 1' AS group_first_value,
    'TIMESTAMP_NTZ "1970-01-01 00:00:01.000"' AS group_last_value,
    4 AS group_distinct_rows,
    5 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":null,"GROUP_CODE":"V-1","SORT_KEY":4,"VALUE_ID":"VV-04"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-06' AS pair_id,
    TRUE AS values_equal,
    TRUE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-07' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-07-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-07-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-08' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-08-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-05~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":1,"VALUE_ID":"VV-05"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-06~VV-07' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":2,"VALUE_ID":"VV-06"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-06~VV-07-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":2,"VALUE_ID":"VV-06"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-06~VV-07-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":2,"VALUE_ID":"VV-06"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-06~VV-08' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":2,"VALUE_ID":"VV-06"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-06~VV-08-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":2,"VALUE_ID":"VV-06"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-06~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":{"a":1,"b":2},"GROUP_CODE":"V-2","SORT_KEY":2,"VALUE_ID":"VV-06"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07-DATE~VV-07-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":103,"VALUE_ID":"VV-07-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07-DATE~VV-08-DATE' AS pair_id,
    TRUE AS values_equal,
    TRUE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":103,"VALUE_ID":"VV-07-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07-DATE~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":103,"VALUE_ID":"VV-07-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07-TIMESTAMP~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01 00:00:00.000' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01 00:00:00.000","GROUP_CODE":"V-2","SORT_KEY":203,'
      || '"VALUE_ID":"VV-07-TIMESTAMP"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07~VV-07-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":3,"VALUE_ID":"VV-07"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07~VV-07-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":3,"VALUE_ID":"VV-07"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07~VV-08' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":3,"VALUE_ID":"VV-07"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07~VV-08-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":3,"VALUE_ID":"VV-07"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-07~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":3,"VALUE_ID":"VV-07"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-08-DATE~VV-07-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":104,"VALUE_ID":"VV-08-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-08-DATE~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-2","SORT_KEY":104,"VALUE_ID":"VV-08-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-08~VV-07-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01 10:15:00' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01 10:15:00","GROUP_CODE":"V-2","SORT_KEY":4,"VALUE_ID":"VV-08"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-08~VV-07-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01 10:15:00' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01 10:15:00","GROUP_CODE":"V-2","SORT_KEY":4,"VALUE_ID":"VV-08"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-08~VV-08-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01 10:15:00' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01 10:15:00","GROUP_CODE":"V-2","SORT_KEY":4,"VALUE_ID":"VV-08"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-08~VV-08-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'OBJECT {"a":1,"b":2}' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 10:15:00.000"' AS group_last_value,
    6 AS group_distinct_rows,
    6 AS group_distinct_elements,
    '2026-03-01 10:15:00' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01 10:15:00","GROUP_CODE":"V-2","SORT_KEY":4,"VALUE_ID":"VV-08"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-09~VV-10' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'ARRAY [1,2]' AS group_first_value,
    'ARRAY [2,1]' AS group_last_value,
    2 AS group_distinct_rows,
    2 AS group_distinct_elements,
    CAST(NULL AS VARCHAR) AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":[1,2],"GROUP_CODE":"V-3","SORT_KEY":1,"VALUE_ID":"VV-09"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-11~VV-12' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'DATE "2026-03-01"' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 00:00:00.000"' AS group_last_value,
    3 AS group_distinct_rows,
    3 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-4","SORT_KEY":1,"VALUE_ID":"VV-11"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-11~VV-12-DATE' AS pair_id,
    TRUE AS values_equal,
    TRUE AS values_equal_null,
    2 AS left_partition_size,
    'DATE "2026-03-01"' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 00:00:00.000"' AS group_last_value,
    3 AS group_distinct_rows,
    3 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-4","SORT_KEY":1,"VALUE_ID":"VV-11"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-11~VV-12-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'DATE "2026-03-01"' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 00:00:00.000"' AS group_last_value,
    3 AS group_distinct_rows,
    3 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-4","SORT_KEY":1,"VALUE_ID":"VV-11"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-12-DATE~VV-12-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    2 AS left_partition_size,
    'DATE "2026-03-01"' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 00:00:00.000"' AS group_last_value,
    3 AS group_distinct_rows,
    3 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-4","SORT_KEY":102,"VALUE_ID":"VV-12-DATE"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-12~VV-12-DATE' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'DATE "2026-03-01"' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 00:00:00.000"' AS group_last_value,
    3 AS group_distinct_rows,
    3 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-4","SORT_KEY":2,"VALUE_ID":"VV-12"}' AS left_object_without_note
  UNION ALL
  SELECT
    'VV-12~VV-12-TIMESTAMP' AS pair_id,
    FALSE AS values_equal,
    FALSE AS values_equal_null,
    1 AS left_partition_size,
    'DATE "2026-03-01"' AS group_first_value,
    'TIMESTAMP_NTZ "2026-03-01 00:00:00.000"' AS group_last_value,
    3 AS group_distinct_rows,
    3 AS group_distinct_elements,
    '2026-03-01' AS left_scalar_text,
    '{"ATTRIBUTE_VALUE":"2026-03-01","GROUP_CODE":"V-4","SORT_KEY":2,"VALUE_ID":"VV-12"}' AS left_object_without_note
)
