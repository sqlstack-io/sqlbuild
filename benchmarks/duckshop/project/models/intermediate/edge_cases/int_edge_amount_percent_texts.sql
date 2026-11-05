MODEL (
  description """
    One row per reconciliation check with a third of its amount as a percentage written with a
    number format of four whole digits
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    amount_percent (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TO_VARCHAR(ROUND(a.amount * 100 / 3, 1), 'FM9990.0') || '%' AS amount_percent
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
