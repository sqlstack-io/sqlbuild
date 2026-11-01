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
    'full:-:AC-02' AS pair_id,
    'full' AS join_kind,
    '12345678901234567890123' AS check_key_text,
    CAST(NULL AS VARCHAR) AS narrow_check_id,
    'AC-02' AS wide_check_id,
    'NUMBER(38,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'full:AC-01:AC-01' AS pair_id,
    'full' AS join_kind,
    '7' AS check_key_text,
    'AC-01' AS narrow_check_id,
    'AC-01' AS wide_check_id,
    'NUMBER(38,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'full:AC-02:AC-03' AS pair_id,
    'full' AS join_kind,
    '12' AS check_key_text,
    'AC-02' AS narrow_check_id,
    'AC-03' AS wide_check_id,
    'NUMBER(38,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'full:AC-04:AC-04' AS pair_id,
    'full' AS join_kind,
    '123456789012345678' AS check_key_text,
    'AC-04' AS narrow_check_id,
    'AC-04' AS wide_check_id,
    'NUMBER(38,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'full:AC-05:-' AS pair_id,
    'full' AS join_kind,
    '99' AS check_key_text,
    'AC-05' AS narrow_check_id,
    CAST(NULL AS VARCHAR) AS wide_check_id,
    'NUMBER(38,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'full:AC-06:AC-06' AS pair_id,
    'full' AS join_kind,
    '5' AS check_key_text,
    'AC-06' AS narrow_check_id,
    'AC-06' AS wide_check_id,
    'NUMBER(38,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'inner:AC-01:AC-01' AS pair_id,
    'inner' AS join_kind,
    '7' AS check_key_text,
    'AC-01' AS narrow_check_id,
    'AC-01' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(18,0)' AS wide_side_type
  UNION ALL
  SELECT
    'inner:AC-02:AC-03' AS pair_id,
    'inner' AS join_kind,
    '12' AS check_key_text,
    'AC-02' AS narrow_check_id,
    'AC-03' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(18,0)' AS wide_side_type
  UNION ALL
  SELECT
    'inner:AC-04:AC-04' AS pair_id,
    'inner' AS join_kind,
    '123456789012345678' AS check_key_text,
    'AC-04' AS narrow_check_id,
    'AC-04' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(18,0)' AS wide_side_type
  UNION ALL
  SELECT
    'inner:AC-06:AC-06' AS pair_id,
    'inner' AS join_kind,
    '5' AS check_key_text,
    'AC-06' AS narrow_check_id,
    'AC-06' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(18,0)' AS wide_side_type
  UNION ALL
  SELECT
    'left:AC-01:AC-01' AS pair_id,
    'left' AS join_kind,
    '7' AS check_key_text,
    'AC-01' AS narrow_check_id,
    'AC-01' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'left:AC-02:AC-03' AS pair_id,
    'left' AS join_kind,
    '12' AS check_key_text,
    'AC-02' AS narrow_check_id,
    'AC-03' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'left:AC-04:AC-04' AS pair_id,
    'left' AS join_kind,
    '123456789012345678' AS check_key_text,
    'AC-04' AS narrow_check_id,
    'AC-04' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'left:AC-05:-' AS pair_id,
    'left' AS join_kind,
    '99' AS check_key_text,
    'AC-05' AS narrow_check_id,
    CAST(NULL AS VARCHAR) AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'left:AC-06:AC-06' AS pair_id,
    'left' AS join_kind,
    '5' AS check_key_text,
    'AC-06' AS narrow_check_id,
    'AC-06' AS wide_check_id,
    'NUMBER(18,0)' AS merged_key_type,
    'NUMBER(18,0)' AS narrow_side_type,
    'NUMBER(38,0)' AS wide_side_type
  UNION ALL
  SELECT
    'text:AC-03:AC-03' AS pair_id,
    'text' AS join_kind,
    '12' AS check_key_text,
    'AC-03' AS narrow_check_id,
    'AC-03' AS wide_check_id,
    'VARCHAR(3)' AS merged_key_type,
    'VARCHAR(3)' AS narrow_side_type,
    'VARCHAR(3)' AS wide_side_type
  UNION ALL
  SELECT
    'text:AC-04:AC-01' AS pair_id,
    'text' AS join_kind,
    ' 7' AS check_key_text,
    'AC-04' AS narrow_check_id,
    'AC-01' AS wide_check_id,
    'VARCHAR(3)' AS merged_key_type,
    'VARCHAR(3)' AS narrow_side_type,
    'VARCHAR(3)' AS wide_side_type
)
