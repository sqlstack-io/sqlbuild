MODEL (
  description """
    One row per reconciliation check with the parts of its key text folded onto its group code
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    folded_key (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    REDUCE(SPLIT(a.key_text, ','), a.group_code, (acc, part) -> acc || '/' || part)::VARCHAR AS folded_key
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
