MODEL (
  description """
    One row per supplier note from a source whose contract is not enforced, with the note upper-
    cased
  """,
  tags [edge_case],
  columns (
    note_id (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_code (type VARCHAR),
    note_upper (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__supplier_notes")
),

final AS (
  SELECT
    s.note_id,
    s.supplier_code,
    UPPER(s.note_text) AS note_upper
  FROM src AS s
)

SELECT *
FROM final
