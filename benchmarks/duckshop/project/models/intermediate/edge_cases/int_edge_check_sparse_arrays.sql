MODEL (
  description """
    One row per reconciliation check with its key text and group code in an array, read back as
    text, where a missing key leaves a hole
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    pair_text (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_CONSTRUCT(a.key_text, a.group_code)::VARCHAR AS pair_text
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
