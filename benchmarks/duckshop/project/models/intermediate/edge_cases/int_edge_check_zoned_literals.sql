MODEL (
  description """
    One row per reconciliation check with a fixed zoned time written in ISO form with a T and
    +00:00
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    cutover_at (type TIMESTAMP_TZ),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    CAST('2026-03-04T12:00:02+00:00' AS TIMESTAMPTZ) AS cutover_at
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
