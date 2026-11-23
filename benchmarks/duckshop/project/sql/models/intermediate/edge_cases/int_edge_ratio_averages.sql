MODEL (
  description """
    One row per check group with the average of its FLOAT ratios, whose summation order the
    engines do not share
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    average_ratio (type FLOAT),
  ),
);

WITH final AS (
  SELECT
    a.group_code,
    AVG(a.ratio)::FLOAT AS average_ratio
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
)

SELECT *
FROM final
