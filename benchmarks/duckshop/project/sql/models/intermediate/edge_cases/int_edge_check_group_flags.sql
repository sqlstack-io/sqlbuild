MODEL (
  description """
    One row per reconciliation check flagging a group code outside the groups with a positive
    amount
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    is_unpaid_group (type BOOLEAN),
  ),
);

WITH paid_groups AS (
  SELECT DISTINCT b.group_code
  FROM __ref("stg_edge_amount_checks") AS b
  WHERE b.amount > 0 AND b.group_code IS NOT NULL
),

final AS (
  SELECT
    a.check_id,
    (a.group_code NOT IN (
      SELECT p.group_code FROM paid_groups AS p
    ))::BOOLEAN AS is_unpaid_group
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
