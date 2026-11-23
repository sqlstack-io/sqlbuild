MODEL (
  description """
    One row per reconciliation check with a fixed time written without seconds and cast to
    TIMESTAMP_NTZ
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    opened_at (type TIMESTAMP_NTZ),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    CAST('2024-05-01 15:00' AS TIMESTAMP_NTZ) AS opened_at
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
