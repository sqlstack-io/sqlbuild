MODEL (
  description """
    One row per reconciliation check with its key text read as a date in a format taken from a
    column
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_date (type DATE),
  ),
);

WITH formats AS (
  SELECT v.date_format
  FROM VALUES ('YYYY-MM-DD') AS v(date_format)
),

final AS (
  SELECT
    a.check_id,
    TRY_TO_DATE(a.key_text, f.date_format) AS key_date
  FROM __ref("stg_edge_amount_checks") AS a
  CROSS JOIN formats AS f
)

SELECT *
FROM final
