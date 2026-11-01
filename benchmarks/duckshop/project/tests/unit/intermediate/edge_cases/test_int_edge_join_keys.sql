TEST (name "int_edge_join_keys__takes_merged_keys_and_outer_sides_from_the_right_relation");

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

__expected__int_edge_join_keys AS (
  SELECT
    'asof:AC-01:G-1:-' AS row_key,
    'asof:AC-01' AS join_shape,
    'G-1' AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'asof:AC-02:G-1:ten or more' AS row_key,
    'asof:AC-02' AS join_shape,
    'G-1' AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    'ten or more' AS threshold_name
  UNION ALL
  SELECT
    'asof:AC-03:G-1:-' AS row_key,
    'asof:AC-03' AS join_shape,
    'G-1' AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'asof:AC-04:G-2:a hundred or more' AS row_key,
    'asof:AC-04' AS join_shape,
    'G-2' AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    'a hundred or more' AS threshold_name
  UNION ALL
  SELECT
    'asof:AC-05:G-2:ten or more' AS row_key,
    'asof:AC-05' AS join_shape,
    'G-2' AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    'ten or more' AS threshold_name
  UNION ALL
  SELECT
    'asof:AC-06:G-3:-' AS row_key,
    'asof:AC-06' AS join_shape,
    'G-3' AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'natural:G-1:-' AS row_key,
    'natural' AS join_shape,
    'G-1' AS group_code,
    'Mallard desk' AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'natural:G-2:-' AS row_key,
    'natural' AS join_shape,
    'G-2' AS group_code,
    'Teal desk' AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'right_using:G-1:-' AS row_key,
    'right_using' AS join_shape,
    'G-1' AS group_code,
    'Mallard desk' AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'right_using:G-2:-' AS row_key,
    'right_using' AS join_shape,
    'G-2' AS group_code,
    'Teal desk' AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'right_using:G-9:-' AS row_key,
    'right_using' AS join_shape,
    'G-9' AS group_code,
    'Eider desk' AS group_name,
    CAST(NULL AS VARCHAR) AS threshold_name
  UNION ALL
  SELECT
    'using_then_right:-:G-7' AS row_key,
    'using_then_right' AS join_shape,
    CAST(NULL AS VARCHAR) AS group_code,
    CAST(NULL AS VARCHAR) AS group_name,
    'G-7' AS threshold_name
  UNION ALL
  SELECT
    'using_then_right:G-1:G-1' AS row_key,
    'using_then_right' AS join_shape,
    'G-1' AS group_code,
    'Mallard desk' AS group_name,
    'G-1' AS threshold_name
)
