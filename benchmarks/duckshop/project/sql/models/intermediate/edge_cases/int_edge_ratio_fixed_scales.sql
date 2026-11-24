MODEL (
  description """
    One row per reconciliation check with its FLOAT ratio cast to NUMBER(38, 10)
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_fixed (type NUMBER(38, 10)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    CAST(a.ratio AS NUMBER(38, 10)) AS ratio_fixed
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
