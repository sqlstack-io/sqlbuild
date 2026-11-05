MODEL (
  description """
    One row per reconciliation check whose key text holds a listed number, kept only for listed
    (check, group) pairs
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    has_seven (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_CONTAINS('7'::VARIANT, SPLIT(a.key_text, ','))::BOOLEAN AS has_seven
  FROM __ref("stg_edge_amount_checks") AS a
  WHERE (a.check_id, a.group_code) IN (('AC-01', 'G-1'), ('AC-03', 'G-2'))
)

SELECT *
FROM final
