TEST (name "int_edge_variant_expected_values__compares_values_with_expected_ones");

WITH __ref__stg_edge_variant_values AS (
  SELECT
    v.value_id::VARCHAR AS value_id,
    v.group_code::VARCHAR AS group_code,
    v.sort_key::NUMBER(38, 0) AS sort_key,
    PARSE_JSON(v.attribute_value) AS attribute_value,
    v.value_text::VARCHAR AS value_text
  FROM VALUES
    ('VV-01', 'V-1', 1, '{"sku": "DK-1", "qty": 2}', '<sku>DK-1</sku>'),
    ('VV-02', 'V-1', 2, '1.5', '1.5'),
    ('VV-03', 'V-2', 1, '"2026-03-01"', '2026-03-01'),
    ('VV-04', 'V-2', 2, 'null', NULL) AS v(value_id, group_code, sort_key, attribute_value, value_text)
),

__expected__int_edge_variant_expected_values AS (
  SELECT
    NULL AS value_id
)
