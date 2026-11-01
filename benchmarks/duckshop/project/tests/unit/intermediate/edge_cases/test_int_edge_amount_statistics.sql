TEST (name "int_edge_amount_statistics__takes_medians_at_every_scale_and_aggregates_over_no_rows");

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
    ('AC-01', 'G-1', 7, NULL, NULL, 10.00, 0.123456789012345, -155.5, NULL),
    ('AC-02', 'G-1', 7, NULL, NULL, 10.00, 0.123456789012344, 0.29, NULL),
    ('AC-03', 'G-2', 12, NULL, NULL, NULL, 1.000000000000001, 2.675, NULL),
    ('AC-04', 'G-3', 5, NULL, NULL, 0.00, 0.000000000000001, 0.57, NULL),
    ('AC-05', 'G-3', 5, NULL, NULL, -0.50, 0.000000000000002, -1.0, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, fine_amount, ratio, json_text)
),

__expected__int_edge_amount_statistics AS (
  SELECT
    NULL AS group_code,
    NULL AS median_scale_0,
    NULL AS median_scale_2,
    NULL AS median_scale_4,
    NULL AS median_scale_10,
    NULL AS median_scale_11,
    NULL AS median_scale_12,
    NULL AS median_scale_15,
    NULL AS mode_amount,
    NULL AS paid_count,
    NULL AS mode_over_no_rows,
    NULL AS median_over_no_rows,
    NULL AS paid_count_over_no_rows,
    NULL AS mode_with_having_only
),

__assert__tied_pick_is_a_zero AS (
  SELECT s.group_code
  FROM __ref("int_edge_amount_statistics") AS s
  WHERE s.signed_zero_pick <> 0
)
