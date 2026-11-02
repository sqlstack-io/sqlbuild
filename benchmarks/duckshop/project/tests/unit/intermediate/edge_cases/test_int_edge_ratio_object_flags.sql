TEST (name "int_edge_ratio_object_flags__reads_flags_back_as_booleans");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id,
    v.ratio::FLOAT AS ratio
  FROM VALUES
    ('AC-01', 0.1),
    ('AC-02', -0.5),
    ('AC-03', NULL) AS v(check_id, ratio)
),

__expected__int_edge_ratio_object_flags AS (
  SELECT
    'AC-01' AS check_id,
    TRUE AS ratio_flag
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    FALSE AS ratio_flag
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    CAST(NULL AS BOOLEAN) AS ratio_flag
)
