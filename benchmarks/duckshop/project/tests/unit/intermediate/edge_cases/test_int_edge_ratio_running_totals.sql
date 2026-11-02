TEST (name "int_edge_ratio_running_totals__sums_float_ratios_in_check_order");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id,
    v.group_code::VARCHAR AS group_code,
    v.ratio::FLOAT AS ratio
  FROM VALUES
    ('AC-01', 'G-1', 0.1),
    ('AC-02', 'G-1', 0.2),
    ('AC-03', 'G-1', 0.3),
    ('AC-04', 'G-2', 1.5) AS v(check_id, group_code, ratio)
),

__expected__int_edge_ratio_running_totals AS (
  SELECT
    NULL AS check_id,
    NULL AS running_ratio
)
