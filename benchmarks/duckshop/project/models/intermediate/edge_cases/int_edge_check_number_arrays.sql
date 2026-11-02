MODEL (
  description """
    One row per reconciliation check with the size of an array holding its amount, cast through
    the structured type ARRAY(NUMBER(38, 10))
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    amount_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_SIZE(CAST(CAST(ARRAY_CONSTRUCT(a.amount) AS ARRAY(NUMBER(38, 10))) AS ARRAY))::NUMBER(38, 0) AS amount_count
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
