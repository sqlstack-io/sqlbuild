MODEL (
  description """
    One row per reconciliation check with its amount in an array cast through the structured
    type ARRAY(NUMBER(38, 10))
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    amounts (type ARRAY),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    CAST(CAST(ARRAY_CONSTRUCT(a.amount) AS ARRAY(NUMBER(38, 10))) AS ARRAY) AS amounts
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
