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
    NULL AS value_id,
    NULL AS largest_element,
    NULL AS smallest_element,
    NULL AS reversed_elements,
    NULL AS group_largest,
    NULL AS without_json_nulls,
    NULL AS distinct_element_count,
    NULL AS overlaps_json_null,
    NULL AS in_decimal_array,
    NULL AS decimal_array_position
)
