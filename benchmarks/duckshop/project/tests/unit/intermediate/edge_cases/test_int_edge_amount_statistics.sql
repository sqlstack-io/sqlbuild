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
    'G-1' AS group_code,
    0.000::NUMBER(38, 3) AS median_scale_0,
    0.12000::NUMBER(38, 5) AS median_scale_2,
    0.1235000::NUMBER(38, 7) AS median_scale_4,
    0.1234567890000::NUMBER(38, 13) AS median_scale_10,
    0.12345678901000::NUMBER(38, 14) AS median_scale_11,
    0.123456789012000::NUMBER(38, 15) AS median_scale_12,
    2 AS paid_count,
    CAST(NULL AS NUMBER(18, 2)) AS mode_over_no_rows,
    CAST(NULL AS NUMBER(38, 5)) AS median_over_no_rows,
    CAST(NULL AS NUMBER(38, 0)) AS paid_count_over_no_rows,
    0 AS mode_with_having_only
  UNION ALL
  SELECT
    'G-2' AS group_code,
    1.000::NUMBER(38, 3) AS median_scale_0,
    1.00000::NUMBER(38, 5) AS median_scale_2,
    1.0000000::NUMBER(38, 7) AS median_scale_4,
    1.0000000000000::NUMBER(38, 13) AS median_scale_10,
    1.00000000000000::NUMBER(38, 14) AS median_scale_11,
    1.000000000000000::NUMBER(38, 15) AS median_scale_12,
    0 AS paid_count,
    CAST(NULL AS NUMBER(18, 2)) AS mode_over_no_rows,
    CAST(NULL AS NUMBER(38, 5)) AS median_over_no_rows,
    CAST(NULL AS NUMBER(38, 0)) AS paid_count_over_no_rows,
    0 AS mode_with_having_only
  UNION ALL
  SELECT
    'G-3' AS group_code,
    0.000::NUMBER(38, 3) AS median_scale_0,
    0.00000::NUMBER(38, 5) AS median_scale_2,
    0.0000000::NUMBER(38, 7) AS median_scale_4,
    0.0000000000000::NUMBER(38, 13) AS median_scale_10,
    0.00000000000000::NUMBER(38, 14) AS median_scale_11,
    0.000000000000000::NUMBER(38, 15) AS median_scale_12,
    0 AS paid_count,
    CAST(NULL AS NUMBER(18, 2)) AS mode_over_no_rows,
    CAST(NULL AS NUMBER(38, 5)) AS median_over_no_rows,
    CAST(NULL AS NUMBER(38, 0)) AS paid_count_over_no_rows,
    0 AS mode_with_having_only
),

-- Ties and scale-15 medians are not compared exactly: G-3's two amounts are each the mode, and
-- Snowflake's MEDIAN above scale 12 is not exact (0.123456789012344496 or ...498 for a midpoint of
-- 0.1234567890123445, depending on the plan).
__assert__picks_and_fine_medians_are_close AS (
  SELECT s.group_code
  FROM __ref("int_edge_amount_statistics") AS s
  WHERE
    s.signed_zero_pick <> 0
    OR (
      s.group_code = 'G-1'
      AND s.mode_amount <> 10.00
    )
    OR (
      s.group_code = 'G-2'
      AND s.mode_amount IS NOT NULL
    )
    OR (
      s.group_code = 'G-3'
      AND s.mode_amount NOT IN (0.00, -0.50)
    )
    OR ABS(s.median_scale_15
    - DECODE(
      s.group_code,
      'G-1', 0.1234567890123445,
      'G-2', 1.000000000000001,
      'G-3', 0.0000000000000015
    )) > 0.00000000000000001
)
