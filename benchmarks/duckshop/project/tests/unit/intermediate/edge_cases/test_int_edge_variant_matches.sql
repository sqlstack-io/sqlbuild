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
    NULL AS pair_id,
    NULL AS values_equal,
    NULL AS values_equal_null,
    NULL AS left_partition_size,
    NULL AS group_first_value,
    NULL AS group_last_value,
    NULL AS group_distinct_rows,
    NULL AS group_distinct_elements,
    NULL AS left_scalar_text,
    NULL AS left_object_without_note
)
