MODEL (
  description """
    One row per reconciliation check with its key text split into an array cast through the
    structured type ARRAY(VARCHAR)
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_parts (type ARRAY),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    CAST(CAST(SPLIT(a.key_text, ',') AS ARRAY(VARCHAR)) AS ARRAY) AS key_parts
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
