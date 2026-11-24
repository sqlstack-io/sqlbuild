MODEL (
  description """
    One row per check group with MEDIAN at scales 0 to 15, where the two middle values differ in
    their last digits; MODE and COUNT_IF per group; and MODE, MEDIAN and COUNT_IF over no rows,
    without GROUP BY and with HAVING but no GROUP BY, which Snowflake answers with one row; MODE of
    a number over no rows can come back as 0, not NULL. Tied modes, the pick of a MAX_BY whose keys
    all tie and MEDIAN above scale 12, which Snowflake does not compute exactly, are checked by
    assertions rather than compared
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    median_scale_0 (type NUMBER(38, 3)),
    median_scale_2 (type NUMBER(38, 5)),
    median_scale_4 (type NUMBER(38, 7)),
    median_scale_10 (type NUMBER(38, 13)),
    median_scale_11 (type NUMBER(38, 14)),
    median_scale_12 (type NUMBER(38, 15)),
    median_scale_15 (type NUMBER(38, 18)),
    mode_amount (type NUMBER(18, 2)),
    paid_count (type NUMBER(38, 0)),
    signed_zero_pick (type FLOAT),
    mode_over_no_rows (type NUMBER(18, 2)),
    median_over_no_rows (type NUMBER(38, 5)),
    paid_count_over_no_rows (type NUMBER(38, 0)),
    mode_with_having_only (type NUMBER(18, 0)),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.group_code,
    c.narrow_key,
    c.amount,
    c.fine_amount,
    c.ratio
  FROM __ref("stg_edge_amount_checks") AS c
),

per_group AS (
  SELECT
    c.group_code,
    MEDIAN(c.fine_amount::NUMBER(20, 0)) AS median_scale_0,
    MEDIAN(c.fine_amount::NUMBER(20, 2)) AS median_scale_2,
    MEDIAN(c.fine_amount::NUMBER(20, 4)) AS median_scale_4,
    MEDIAN(c.fine_amount::NUMBER(20, 10)) AS median_scale_10,
    MEDIAN(c.fine_amount::NUMBER(20, 11)) AS median_scale_11,
    MEDIAN(c.fine_amount::NUMBER(20, 12)) AS median_scale_12,
    MEDIAN(c.fine_amount) AS median_scale_15,
    MODE(c.amount) AS mode_amount,
    COUNT_IF(c.amount > 0) AS paid_count,
    -- Every key ties, so either row's value is a valid pick; the test checks only that it is zero.
    MAX_BY(c.ratio * 0, c.group_code) AS signed_zero_pick
  FROM checks AS c
  GROUP BY
    c.group_code
),

over_no_rows AS (
  SELECT
    MODE(c.amount) AS mode_over_no_rows,
    MEDIAN(c.amount) AS median_over_no_rows,
    COUNT_IF(c.amount > 0) AS paid_count_over_no_rows
  FROM checks AS c
  WHERE c.check_id IS NULL
),

having_only AS (
  SELECT MODE(c.narrow_key) AS mode_with_having_only
  FROM checks AS c
  WHERE c.check_id IS NULL
  HAVING COUNT(*) >= 0
),

final AS (
  SELECT
    g.group_code,
    g.median_scale_0::NUMBER(38, 3) AS median_scale_0,
    g.median_scale_2::NUMBER(38, 5) AS median_scale_2,
    g.median_scale_4::NUMBER(38, 7) AS median_scale_4,
    g.median_scale_10::NUMBER(38, 13) AS median_scale_10,
    g.median_scale_11::NUMBER(38, 14) AS median_scale_11,
    g.median_scale_12::NUMBER(38, 15) AS median_scale_12,
    g.median_scale_15::NUMBER(38, 18) AS median_scale_15,
    g.mode_amount::NUMBER(18, 2) AS mode_amount,
    g.paid_count::NUMBER(38, 0) AS paid_count,
    g.signed_zero_pick::FLOAT AS signed_zero_pick,
    n.mode_over_no_rows::NUMBER(18, 2) AS mode_over_no_rows,
    n.median_over_no_rows::NUMBER(38, 5) AS median_over_no_rows,
    n.paid_count_over_no_rows::NUMBER(38, 0) AS paid_count_over_no_rows,
    h.mode_with_having_only::NUMBER(18, 0) AS mode_with_having_only
  FROM per_group AS g
  CROSS JOIN over_no_rows AS n
  CROSS JOIN having_only AS h
)

SELECT *
FROM final
