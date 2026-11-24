MODEL (
  description """
    One row per check group with its FLOAT ratios in an object keyed by check, and the number of
    distinct ratios held as VARIANT
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_json (type VARCHAR),
    distinct_ratio_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.group_code,
    TO_JSON(OBJECT_AGG(a.check_id, a.ratio)) AS ratio_json,
    COUNT(DISTINCT TO_VARIANT(a.ratio))::NUMBER(38, 0) AS distinct_ratio_count
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
)

SELECT *
FROM final
