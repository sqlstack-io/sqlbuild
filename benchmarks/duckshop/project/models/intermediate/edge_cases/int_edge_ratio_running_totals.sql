MODEL (
  description """
    One row per reconciliation check with a running total of its FLOAT ratio within its group, a
    sum whose last bits depend on the order of addition
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    running_ratio (type FLOAT),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    SUM(a.ratio) OVER (
      PARTITION BY a.group_code
      ORDER BY a.check_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::FLOAT AS running_ratio
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
