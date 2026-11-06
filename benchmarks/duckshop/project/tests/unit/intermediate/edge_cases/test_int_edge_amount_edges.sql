TEST (name "int_edge_amount_edges__divides_by_zeros_and_truncates_float_ratios");

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
    ('AC-01', 'G-1', NULL, NULL, '\u0130nf', 10.00, NULL, 0.29, NULL),
    ('AC-02', 'G-1', NULL, NULL, 'inf', NULL, NULL, 1.005, NULL),
    ('AC-03', 'G-1', NULL, NULL, 'NaN', 2.50, NULL, 2.675, NULL),
    ('AC-04', 'G-2', NULL, NULL, '-Infinity', -0.50, NULL, -155.5, NULL),
    ('AC-05', 'G-2', NULL, NULL, 'n/a', 0.00, NULL, 0.57, NULL),
    ('AC-06', 'G-2', NULL, NULL, '1e3', 4.00, NULL, 3.0, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, fine_amount, ratio, json_text)
),

__expected__int_edge_amount_edges AS (
  SELECT
    'AC-01' AS check_id,
    'G-1' AS group_code,
    0.000000::NUMBER(38, 6) AS one_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_scaled_zero,
    0.000000::NUMBER(38, 6) AS amount_over_minus_zero,
    0.000000::NUMBER(38, 6) AS amount_over_null,
    CAST(NULL AS NUMBER(38, 6)) AS null_over_zero,
    '0.28'::FLOAT AS ratio_truncated_2,
    '0.29'::FLOAT AS ratio_truncated_3,
    '0.29' AS ratio_text,
    '3.141592654' AS pi_text,
    CAST(NULL AS FLOAT) AS key_text_as_double,
    10.00::NUMBER(18, 2) AS same_amount_lag
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    'G-1' AS group_code,
    0.000000::NUMBER(38, 6) AS one_over_zero,
    CAST(NULL AS NUMBER(38, 6)) AS amount_over_zero,
    CAST(NULL AS NUMBER(38, 6)) AS amount_over_scaled_zero,
    CAST(NULL AS NUMBER(38, 6)) AS amount_over_minus_zero,
    CAST(NULL AS NUMBER(38, 6)) AS amount_over_null,
    CAST(NULL AS NUMBER(38, 6)) AS null_over_zero,
    '1.0'::FLOAT AS ratio_truncated_2,
    '1.004'::FLOAT AS ratio_truncated_3,
    '1.005' AS ratio_text,
    '3.141592654' AS pi_text,
    'inf'::FLOAT AS key_text_as_double,
    CAST(NULL AS NUMBER(18, 2)) AS same_amount_lag
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    'G-1' AS group_code,
    0.000000::NUMBER(38, 6) AS one_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_scaled_zero,
    0.000000::NUMBER(38, 6) AS amount_over_minus_zero,
    0.000000::NUMBER(38, 6) AS amount_over_null,
    CAST(NULL AS NUMBER(38, 6)) AS null_over_zero,
    '2.67'::FLOAT AS ratio_truncated_2,
    '2.675'::FLOAT AS ratio_truncated_3,
    '2.675' AS ratio_text,
    '3.141592654' AS pi_text,
    'nan'::FLOAT AS key_text_as_double,
    2.50::NUMBER(18, 2) AS same_amount_lag
  UNION ALL
  SELECT
    'AC-04' AS check_id,
    'G-2' AS group_code,
    0.000000::NUMBER(38, 6) AS one_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_scaled_zero,
    0.000000::NUMBER(38, 6) AS amount_over_minus_zero,
    0.000000::NUMBER(38, 6) AS amount_over_null,
    CAST(NULL AS NUMBER(38, 6)) AS null_over_zero,
    '-155.5'::FLOAT AS ratio_truncated_2,
    '-155.5'::FLOAT AS ratio_truncated_3,
    '-155.5' AS ratio_text,
    '3.141592654' AS pi_text,
    '-inf'::FLOAT AS key_text_as_double,
    -0.50::NUMBER(18, 2) AS same_amount_lag
  UNION ALL
  SELECT
    'AC-05' AS check_id,
    'G-2' AS group_code,
    0.000000::NUMBER(38, 6) AS one_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_scaled_zero,
    0.000000::NUMBER(38, 6) AS amount_over_minus_zero,
    0.000000::NUMBER(38, 6) AS amount_over_null,
    CAST(NULL AS NUMBER(38, 6)) AS null_over_zero,
    '0.56'::FLOAT AS ratio_truncated_2,
    '0.57'::FLOAT AS ratio_truncated_3,
    '0.57' AS ratio_text,
    '3.141592654' AS pi_text,
    CAST(NULL AS FLOAT) AS key_text_as_double,
    0.00::NUMBER(18, 2) AS same_amount_lag
  UNION ALL
  SELECT
    'AC-06' AS check_id,
    'G-2' AS group_code,
    0.000000::NUMBER(38, 6) AS one_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_zero,
    0.000000::NUMBER(38, 6) AS amount_over_scaled_zero,
    0.000000::NUMBER(38, 6) AS amount_over_minus_zero,
    0.000000::NUMBER(38, 6) AS amount_over_null,
    CAST(NULL AS NUMBER(38, 6)) AS null_over_zero,
    '3.0'::FLOAT AS ratio_truncated_2,
    '3.0'::FLOAT AS ratio_truncated_3,
    '3' AS ratio_text,
    '3.141592654' AS pi_text,
    '1000.0'::FLOAT AS key_text_as_double,
    4.00::NUMBER(18, 2) AS same_amount_lag
)
