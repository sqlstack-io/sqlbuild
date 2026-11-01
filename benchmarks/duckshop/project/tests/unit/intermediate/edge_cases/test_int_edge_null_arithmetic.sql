TEST (name "int_edge_null_arithmetic__types_null_arithmetic_and_literals");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id,
    v.group_code,
    v.narrow_key::NUMBER(18, 0) AS narrow_key,
    v.wide_key::NUMBER(38, 0) AS wide_key,
    v.key_text,
    v.amount::NUMBER(18, 2) AS amount,
    v.fine_amount::NUMBER(20, 15) AS fine_amount,
    v.ratio::FLOAT AS ratio,
    v.json_text
  FROM VALUES
    ('AC-01', 'G-1', 7, '7', '01', 10.00, 0.123456789012345, 0.29, NULL),
    ('AC-02', 'G-1', 12, '12345678901234567890123', '1', -0.50, 0.123456789012344, 1.005, NULL),
    ('AC-03', 'G-1', NULL, '12', '12', 2.50, 0.5, 2.675, NULL),
    ('AC-04', 'G-2', 123456789012345678, '123456789012345678', ' 7', 0.00, 1, -155.5, NULL),
    ('AC-05', 'G-2', 99, NULL, NULL, NULL, NULL, NULL, NULL),
    ('AC-06', 'G-3', 5, '5', 'n/a', 999999999999.99, 0.000000000000001, 0.57, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, fine_amount, ratio, json_text)
),

__expected__int_edge_null_arithmetic AS (
  SELECT
    NULL AS check_id,
    NULL AS null_plus_amount,
    NULL AS null_times_amount,
    NULL AS null_over_amount,
    NULL AS ratio_over_null,
    NULL AS wide_amount_times_null,
    NULL AS missing_times_amount,
    NULL AS null_times_amount_or_zero,
    NULL AS null_over_amount_type,
    NULL AS ratio_over_null_type,
    NULL AS wide_amount_times_null_type,
    NULL AS missing_times_amount_type,
    NULL AS amount_or_decimal,
    NULL AS amount_or_decimal_type,
    NULL AS exponent_type,
    NULL AS small_exponent_product_type,
    NULL AS remainder_type,
    NULL AS key_remainder_type,
    NULL AS nullif_type,
    NULL AS ilike_null_type,
    NULL AS row_number_type,
    NULL AS count_type,
    NULL AS union_decimal_type,
    NULL AS all_null_union_type,
    NULL AS case_without_else,
    NULL AS decoded
)
