MODEL (
  description """
    One row per reconciliation check and source, with a note column that is NULL in every branch
    of a UNION ALL BY NAME
  """,
  tags [edge_case],
  columns (
    note_row_id (type VARCHAR, nullable false, audits [not_null, unique]),
    note (type VARCHAR),
  ),
);

WITH notes AS (
  SELECT a.check_id || '~left' AS note_row_id, NULL AS note
  FROM __ref("stg_edge_amount_checks") AS a
  UNION ALL BY NAME
  SELECT a.check_id || '~right' AS note_row_id, NULL AS note
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    u.note_row_id,
    u.note::VARCHAR AS note
  FROM notes AS u
)

SELECT *
FROM final
