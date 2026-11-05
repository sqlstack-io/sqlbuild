MODEL (
  description """
    One row per reconciliation check with its amount pivoted into one column per check group
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    first_group_amount (type NUMBER(38, 2)),
    second_group_amount (type NUMBER(38, 2)),
  ),
);

WITH amounts AS (
  SELECT a.check_id, a.group_code, a.amount
  FROM __ref("stg_edge_amount_checks") AS a
),

pivoted AS (
  SELECT *
  FROM amounts
    PIVOT(SUM(amount) FOR group_code IN ('G-1', 'G-2'))
    AS pv (check_id, first_group_amount, second_group_amount)
),

final AS (
  SELECT
    p.check_id::VARCHAR AS check_id,
    p.first_group_amount::NUMBER(38, 2) AS first_group_amount,
    p.second_group_amount::NUMBER(38, 2) AS second_group_amount
  FROM pivoted AS p
)

SELECT *
FROM final
