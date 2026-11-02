TEST (name "int_edge_grouped_windows__ranks_groups_by_aggregates_and_rolls_up_a_null_total");

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
    ('AC-01', 'G-1', NULL, NULL, NULL, 10.00, NULL, NULL, NULL),
    ('AC-02', 'G-1', NULL, NULL, NULL, -0.50, NULL, NULL, NULL),
    ('AC-03', 'G-1', NULL, NULL, NULL, 2.50, NULL, NULL, NULL),
    ('AC-04', 'G-2', NULL, NULL, NULL, 0.00, NULL, NULL, NULL),
    ('AC-05', 'G-2', NULL, NULL, NULL, NULL, NULL, NULL, NULL),
    ('AC-06', 'G-3', NULL, NULL, NULL, 999999999999.99, NULL, NULL, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, fine_amount, ratio, json_text)
),

__expected__int_edge_grouped_windows AS (
  SELECT
    'ALL' AS group_label,
    CAST(NULL AS VARCHAR) AS group_code,
    1000000000011.99::NUMBER(38, 2) AS group_amount,
    6 AS check_count,
    CAST(NULL AS NUMBER(38, 2)) AS total_amount,
    CAST(NULL AS NUMBER(38, 6)) AS amount_share,
    CAST(NULL AS NUMBER(38, 0)) AS size_rank,
    FALSE AS is_largest_group
  UNION ALL
  SELECT
    'G-1' AS group_label,
    'G-1' AS group_code,
    12.00::NUMBER(38, 2) AS group_amount,
    3 AS check_count,
    1000000000011.99::NUMBER(38, 2) AS total_amount,
    0.000000::NUMBER(38, 6) AS amount_share,
    1 AS size_rank,
    FALSE AS is_largest_group
  UNION ALL
  SELECT
    'G-2' AS group_label,
    'G-2' AS group_code,
    0.00::NUMBER(38, 2) AS group_amount,
    2 AS check_count,
    1000000000011.99::NUMBER(38, 2) AS total_amount,
    0.000000::NUMBER(38, 6) AS amount_share,
    2 AS size_rank,
    FALSE AS is_largest_group
  UNION ALL
  SELECT
    'G-3' AS group_label,
    'G-3' AS group_code,
    999999999999.99::NUMBER(38, 2) AS group_amount,
    1 AS check_count,
    1000000000011.99::NUMBER(38, 2) AS total_amount,
    1.000000::NUMBER(38, 6) AS amount_share,
    3 AS size_rank,
    TRUE AS is_largest_group
)
