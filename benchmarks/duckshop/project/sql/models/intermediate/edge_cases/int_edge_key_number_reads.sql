MODEL (
  description """
    One row per reconciliation check with its wide key read back with TO_NUMBER
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    wide_key_number (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TO_NUMBER(a.wide_key)::NUMBER(38, 0) AS wide_key_number
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
