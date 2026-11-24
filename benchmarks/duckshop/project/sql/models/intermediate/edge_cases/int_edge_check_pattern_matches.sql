MODEL (
  description """
    One row per reconciliation check with whether its key text matches its group code read as a
    pattern
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    matches_group (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    REGEXP_LIKE(a.key_text, a.group_code)::BOOLEAN AS matches_group
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
