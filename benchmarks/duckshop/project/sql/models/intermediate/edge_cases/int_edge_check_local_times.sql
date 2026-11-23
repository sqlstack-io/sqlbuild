MODEL (
  description """
    One row per reconciliation check with a fixed local time written as text and cast to
    TIMESTAMP_LTZ
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    noted_at (type TIMESTAMP_LTZ),
  ),
);

WITH noted AS (
  SELECT v.noted_text
  FROM VALUES ('2026-03-04 12:00:00') AS v(noted_text)
),

final AS (
  SELECT
    a.check_id,
    CAST(c.noted_text AS TIMESTAMP_LTZ) AS noted_at
  FROM __ref("stg_edge_amount_checks") AS a
  CROSS JOIN noted AS c
)

SELECT *
FROM final
