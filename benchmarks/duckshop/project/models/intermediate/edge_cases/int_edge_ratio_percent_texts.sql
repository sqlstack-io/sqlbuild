MODEL (
  description """
    One row per reconciliation check with its FLOAT ratio as a percentage written with a number
    format
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_percent (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TO_VARCHAR(ROUND(a.ratio * 100, 1), 'FM9990.0') || '%' AS ratio_percent
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
