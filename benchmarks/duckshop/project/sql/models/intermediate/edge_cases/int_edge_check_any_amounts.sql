MODEL (
  description """
    One row per check group with any one of its amounts, and whether all amounts in the group
    agree
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    amounts_agree (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    a.group_code,
    (COUNT(DISTINCT a.amount) <= 1 OR ANY_VALUE(a.amount) IS NOT NULL)::BOOLEAN AS amounts_agree
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
)

SELECT *
FROM final
