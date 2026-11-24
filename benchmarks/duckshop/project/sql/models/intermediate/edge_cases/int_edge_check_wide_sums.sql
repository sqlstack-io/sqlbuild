MODEL (
  description """
    One row per reconciliation check and value kind, the amount plus one beside the wide key in
    one UNION ALL column
  """,
  tags [edge_case],
  columns (
    key_row_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_value (type NUMBER(38, 2)),
  ),
);

WITH final AS (
  SELECT
    a.check_id || '~amount' AS key_row_id,
    a.amount + 1 AS key_value
  FROM __ref("stg_edge_amount_checks") AS a
  UNION ALL
  SELECT
    a.check_id || '~wide' AS key_row_id,
    a.wide_key AS key_value
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
