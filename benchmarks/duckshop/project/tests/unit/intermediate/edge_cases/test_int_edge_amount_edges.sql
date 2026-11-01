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
    NULL AS check_id,
    NULL AS group_code,
    NULL AS one_over_zero,
    NULL AS amount_over_zero,
    NULL AS amount_over_scaled_zero,
    NULL AS amount_over_minus_zero,
    NULL AS amount_over_null,
    NULL AS null_over_zero,
    NULL AS ratio_truncated_2,
    NULL AS ratio_truncated_3,
    NULL AS ratio_text,
    NULL AS pi_text,
    NULL AS key_text_as_double,
    NULL AS same_amount_lag
),

__assert__running_count_ends_at_the_group_total AS (
  SELECT e.group_code
  FROM __ref("int_edge_amount_edges") AS e
  GROUP BY
    e.group_code
  HAVING MAX(e.running_paid_count) <> IFF(e.group_code = 'G-1', 2, 1)
)
