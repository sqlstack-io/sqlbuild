TEST (name "int_edge_amount_bit_reads__floors_masks_and_compares_amounts_and_keys");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id,
    v.group_code,
    v.narrow_key::NUMBER(18, 0) AS narrow_key,
    v.wide_key::NUMBER(38, 0) AS wide_key,
    NULL::VARCHAR AS key_text,
    v.amount::NUMBER(18, 2) AS amount,
    NULL::NUMBER(20, 15) AS fine_amount,
    v.ratio::FLOAT AS ratio,
    NULL::VARCHAR AS json_text
  FROM VALUES
    ('AC-01', 'G-1', 7, 7, 10.05, -2.5),
    ('AC-02', 'G-1', 12, NULL, -0.55, 2.5),
    ('AC-03', 'G-2', NULL, NULL, NULL, 0.29),
    ('AC-04', 'G-2', 5, 6, 0.00, NULL),
    ('AC-05', 'G-3', 13, 13, 999.99, -0.0) AS v(check_id, group_code, narrow_key, wide_key, amount, ratio)
),

__expected__int_edge_amount_bit_reads AS (
  SELECT
    'AC-01' AS check_id,
    '-3.0'::FLOAT AS ratio_floor,
    10.0::NUMBER(38, 1) AS amount_floor_tenths,
    6 AS narrow_key_bits,
    10.05::NUMBER(18, 2) AS amount_or_zero,
    7 AS narrow_key_bigint,
    '10.05'::FLOAT AS amount_double,
    TRUE AS keys_not_distinct,
    TRUE AS keys_equal_null,
    'Mallard desk' AS group_name,
    2 AS group_check_count,
    TRUE AS group_hash_is_order_free
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    '2.0'::FLOAT AS ratio_floor,
    -0.6::NUMBER(38, 1) AS amount_floor_tenths,
    4 AS narrow_key_bits,
    -0.55::NUMBER(18, 2) AS amount_or_zero,
    12 AS narrow_key_bigint,
    '-0.55'::FLOAT AS amount_double,
    FALSE AS keys_not_distinct,
    FALSE AS keys_equal_null,
    'Mallard desk' AS group_name,
    2 AS group_check_count,
    TRUE AS group_hash_is_order_free
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    '0.0'::FLOAT AS ratio_floor,
    CAST(NULL AS NUMBER(38, 1)) AS amount_floor_tenths,
    CAST(NULL AS NUMBER(38, 0)) AS narrow_key_bits,
    0.00::NUMBER(18, 2) AS amount_or_zero,
    CAST(NULL AS NUMBER(38, 0)) AS narrow_key_bigint,
    CAST(NULL AS FLOAT) AS amount_double,
    TRUE AS keys_not_distinct,
    TRUE AS keys_equal_null,
    'Teal desk' AS group_name,
    2 AS group_check_count,
    TRUE AS group_hash_is_order_free
  UNION ALL
  SELECT
    'AC-04' AS check_id,
    CAST(NULL AS FLOAT) AS ratio_floor,
    0.0::NUMBER(38, 1) AS amount_floor_tenths,
    4 AS narrow_key_bits,
    0.00::NUMBER(18, 2) AS amount_or_zero,
    5 AS narrow_key_bigint,
    '0.0'::FLOAT AS amount_double,
    FALSE AS keys_not_distinct,
    FALSE AS keys_equal_null,
    'Teal desk' AS group_name,
    2 AS group_check_count,
    TRUE AS group_hash_is_order_free
  UNION ALL
  SELECT
    'AC-05' AS check_id,
    '0.0'::FLOAT AS ratio_floor,
    999.9::NUMBER(38, 1) AS amount_floor_tenths,
    4 AS narrow_key_bits,
    999.99::NUMBER(18, 2) AS amount_or_zero,
    13 AS narrow_key_bigint,
    '999.99'::FLOAT AS amount_double,
    TRUE AS keys_not_distinct,
    TRUE AS keys_equal_null,
    'Eider desk' AS group_name,
    1 AS group_check_count,
    TRUE AS group_hash_is_order_free
)
