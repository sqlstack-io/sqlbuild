MODEL (
  description """
    One row per reconciliation check with its key text read as a timestamp, and its FLOAT ratio
    written with a comma by REPLACE
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_time (type TIMESTAMP_NTZ),
    ratio_text (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TRY_CAST(a.key_text AS TIMESTAMP) AS key_time,
    REPLACE(a.ratio, '.', ',') AS ratio_text
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
