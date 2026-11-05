MODEL (
  description """
    One row per reconciliation check with its FLOAT ratio in an array cast to ARRAY
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_SIZE(CAST(ARRAY_CONSTRUCT(a.ratio) AS ARRAY))::NUMBER(38, 0) AS ratio_count
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
