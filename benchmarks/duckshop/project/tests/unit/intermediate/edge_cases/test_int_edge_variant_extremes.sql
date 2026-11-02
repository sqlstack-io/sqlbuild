TEST (name "int_edge_variant_extremes__orders_mixed_variant_values");

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
    ('VV-01', 'V-1', 1, '[1, "a", 2]', NULL),
    ('VV-02', 'V-1', 2, '[1, null, 3]', NULL),
    ('VV-03', 'V-1', 3, '[true, 2]', NULL),
    ('VV-04', 'V-2', 1, '["a", "B", "b"]', NULL),
    ('VV-05', 'V-2', 2, '[12345678901234567890, 1]', NULL),
    ('VV-06', 'V-2', 3, '[9007199254740993, 9007199254740992]', NULL),
    ('VV-07', 'V-3', 1, '[-0.0, 0]', NULL),
    ('VV-08', 'V-3', 2, '[1e2, 99]', NULL),
    ('VV-09', 'V-3', 3, '[]', NULL),
    ('VV-10', 'V-4', 1, 'null', NULL),
    ('VV-11', 'V-4', 2, '5', NULL),
    ('VV-12', 'V-4', 3, '"x"', NULL),
    ('VV-13', 'V-4', 4, '{"a": 1}', NULL),
    ('VV-14', 'V-5', 1, '["\u00e9", "z", "b", "B"]', NULL),
    ('VV-15', 'V-5', 2, '"\u00e9"', NULL),
    ('VV-16', 'V-5', 3, '"z"', NULL),
    ('VV-17', 'V-5', 4, '"B"', NULL),
    ('VV-18', 'V-6', 1, '-7', NULL),
    ('VV-19', 'V-6', 2, '1', NULL),
    ('VV-20', 'V-6', 3, '"1"', NULL),
    ('VV-21', 'V-6', 4, '0', NULL) AS v(value_id, group_code, sort_key, attribute_value, value_text)
),

__expected__int_edge_variant_extremes AS (
  SELECT
    'VV-01' AS value_id,
    PARSE_JSON('"a"') AS largest_element,
    PARSE_JSON('1') AS smallest_element,
    PARSE_JSON('[2,"a",1]')::ARRAY AS reversed_elements,
    PARSE_JSON('[1,null,3]') AS group_largest,
    PARSE_JSON('[1,"a",2]')::ARRAY AS without_json_nulls,
    3 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-02' AS value_id,
    PARSE_JSON('null') AS largest_element,
    PARSE_JSON('1') AS smallest_element,
    PARSE_JSON('[3,null,1]')::ARRAY AS reversed_elements,
    PARSE_JSON('[1,null,3]') AS group_largest,
    PARSE_JSON('[1,3]')::ARRAY AS without_json_nulls,
    3 AS distinct_element_count,
    TRUE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-03' AS value_id,
    PARSE_JSON('2') AS largest_element,
    PARSE_JSON('true') AS smallest_element,
    PARSE_JSON('[2,true]')::ARRAY AS reversed_elements,
    PARSE_JSON('[1,null,3]') AS group_largest,
    PARSE_JSON('[true,2]')::ARRAY AS without_json_nulls,
    2 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-04' AS value_id,
    PARSE_JSON('"b"') AS largest_element,
    PARSE_JSON('"B"') AS smallest_element,
    PARSE_JSON('["b","B","a"]')::ARRAY AS reversed_elements,
    PARSE_JSON('["a","B","b"]') AS group_largest,
    PARSE_JSON('["a","B","b"]')::ARRAY AS without_json_nulls,
    3 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-05' AS value_id,
    PARSE_JSON('12345678901234567890') AS largest_element,
    PARSE_JSON('1') AS smallest_element,
    PARSE_JSON('[1,12345678901234567890]')::ARRAY AS reversed_elements,
    PARSE_JSON('["a","B","b"]') AS group_largest,
    PARSE_JSON('[12345678901234567890,1]')::ARRAY AS without_json_nulls,
    2 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-06' AS value_id,
    PARSE_JSON('9007199254740993') AS largest_element,
    PARSE_JSON('9007199254740992') AS smallest_element,
    PARSE_JSON('[9007199254740992,9007199254740993]')::ARRAY AS reversed_elements,
    PARSE_JSON('["a","B","b"]') AS group_largest,
    PARSE_JSON('[9007199254740993,9007199254740992]')::ARRAY AS without_json_nulls,
    2 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-07' AS value_id,
    PARSE_JSON('0') AS largest_element,
    PARSE_JSON('0') AS smallest_element,
    PARSE_JSON('[0,0]')::ARRAY AS reversed_elements,
    PARSE_JSON('[1.000000000000000e+02,99]') AS group_largest,
    PARSE_JSON('[0,0]')::ARRAY AS without_json_nulls,
    1 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-08' AS value_id,
    PARSE_JSON('1.000000000000000e+02') AS largest_element,
    PARSE_JSON('99') AS smallest_element,
    PARSE_JSON('[99,1.000000000000000e+02]')::ARRAY AS reversed_elements,
    PARSE_JSON('[1.000000000000000e+02,99]') AS group_largest,
    PARSE_JSON('[1.000000000000000e+02,99]')::ARRAY AS without_json_nulls,
    2 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-09' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    PARSE_JSON('[]')::ARRAY AS reversed_elements,
    PARSE_JSON('[1.000000000000000e+02,99]') AS group_largest,
    PARSE_JSON('[]')::ARRAY AS without_json_nulls,
    0 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-10' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('null') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-11' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('null') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-12' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('null') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-13' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('null') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-14' AS value_id,
    PARSE_JSON('"é"') AS largest_element,
    PARSE_JSON('"B"') AS smallest_element,
    PARSE_JSON('["B","b","z","é"]')::ARRAY AS reversed_elements,
    PARSE_JSON('["é","z","b","B"]') AS group_largest,
    PARSE_JSON('["é","z","b","B"]')::ARRAY AS without_json_nulls,
    4 AS distinct_element_count,
    FALSE AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-15' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('["é","z","b","B"]') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-16' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('["é","z","b","B"]') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-17' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('["é","z","b","B"]') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-18' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('"1"') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    TRUE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-19' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('"1"') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    TRUE AS in_decimal_array,
    1 AS decimal_array_position
  UNION ALL
  SELECT
    'VV-20' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('"1"') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    CAST(NULL AS NUMBER(38, 0)) AS decimal_array_position
  UNION ALL
  SELECT
    'VV-21' AS value_id,
    CAST(NULL AS VARIANT) AS largest_element,
    CAST(NULL AS VARIANT) AS smallest_element,
    CAST(NULL AS ARRAY) AS reversed_elements,
    PARSE_JSON('"1"') AS group_largest,
    CAST(NULL AS ARRAY) AS without_json_nulls,
    CAST(NULL AS NUMBER(38, 0)) AS distinct_element_count,
    CAST(NULL AS BOOLEAN) AS overlaps_json_null,
    FALSE AS in_decimal_array,
    2 AS decimal_array_position
)
