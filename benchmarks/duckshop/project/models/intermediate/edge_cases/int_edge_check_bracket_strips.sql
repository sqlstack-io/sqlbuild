MODEL (
  description """
    One row per reconciliation check with bracketed text removed from its key by a lazy pattern
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    stripped_key (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TRIM(REGEXP_REPLACE(a.key_text, '\\(.*?\\)', '')) AS stripped_key
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
