MODEL (
  description """
    One row per check with numeric edges: DIV0 and DIV0NULL by literal zeros and NULL, TRUNC of
    FLOAT ratios that sit just below a whole number of cents, FLOAT values as text (pi among them),
    infinity and NaN words in other scripts read by TRY_TO_DOUBLE, and LAG with offset 0 and IGNORE
    NULLS. A running COUNT_IF without ORDER BY is not compared row by row, since Snowflake may
    number the rows in any order; the test checks only its total per group
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    group_code (type VARCHAR),
    one_over_zero (type NUMBER(38, 6)),
    amount_over_zero (type NUMBER(38, 6)),
    amount_over_scaled_zero (type NUMBER(38, 6)),
    amount_over_minus_zero (type NUMBER(38, 6)),
    amount_over_null (type NUMBER(38, 6)),
    null_over_zero (type NUMBER(38, 6)),
    ratio_truncated_2 (type FLOAT),
    ratio_truncated_3 (type FLOAT),
    ratio_text (type VARCHAR),
    pi_text (type VARCHAR),
    key_text_as_double (type FLOAT),
    same_amount_lag (type NUMBER(18, 2)),
    running_paid_count (type NUMBER(38, 0)),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.group_code,
    c.key_text,
    c.amount,
    c.ratio
  FROM __ref("stg_edge_amount_checks") AS c
),

final AS (
  SELECT
    c.check_id,
    c.group_code,
    DIV0(1, 0)::NUMBER(38, 6) AS one_over_zero,
    DIV0(c.amount, 0)::NUMBER(38, 6) AS amount_over_zero,
    DIV0(c.amount, 0.00)::NUMBER(38, 6) AS amount_over_scaled_zero,
    DIV0(c.amount, -0)::NUMBER(38, 6) AS amount_over_minus_zero,
    DIV0NULL(c.amount, NULL)::NUMBER(38, 6) AS amount_over_null,
    DIV0(NULL, 0)::NUMBER(38, 6) AS null_over_zero,
    TRUNC(c.ratio, 2)::FLOAT AS ratio_truncated_2,
    TRUNCATE(c.ratio, 3)::FLOAT AS ratio_truncated_3,
    c.ratio::VARCHAR AS ratio_text,
    PI()::VARCHAR AS pi_text,
    TRY_TO_DOUBLE(c.key_text)::FLOAT AS key_text_as_double,
    LAG(c.amount, 0) IGNORE NULLS OVER (ORDER BY c.check_id)::NUMBER(18, 2) AS same_amount_lag,
    -- No ORDER BY: each row's count depends on the order Snowflake reads the group in.
    COUNT_IF(c.amount > 0) OVER (
      PARTITION BY c.group_code
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::NUMBER(38, 0) AS running_paid_count
  FROM checks AS c
)

SELECT *
FROM final
