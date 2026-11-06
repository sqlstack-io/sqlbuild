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
    'AC-01' AS check_id,
    '0.1'::FLOAT AS running_ratio
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    '0.30000000000000004'::FLOAT AS running_ratio
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    '0.6'::FLOAT AS running_ratio
  UNION ALL
  SELECT
    'AC-04' AS check_id,
    '1.5'::FLOAT AS running_ratio
)
