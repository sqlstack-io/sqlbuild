TEST (name "int_edge_key_widths__joins_number_widths_and_text_keys_with_using");

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

__expected__int_edge_key_widths AS (
  SELECT
    NULL AS pair_id,
    NULL AS join_kind,
    NULL AS check_key_text,
    NULL AS narrow_check_id,
    NULL AS wide_check_id,
    NULL AS merged_key_type,
    NULL AS narrow_side_type,
    NULL AS wide_side_type
)
