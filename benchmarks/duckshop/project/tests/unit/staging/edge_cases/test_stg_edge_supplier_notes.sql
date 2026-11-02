TEST (name "stg_edge_supplier_notes__upper_cases_supplier_notes");

WITH __source__edge__supplier_notes AS (
  SELECT
    v.note_id::VARCHAR AS note_id,
    v.supplier_code::VARCHAR AS supplier_code,
    v.note_text::VARCHAR AS note_text
  FROM VALUES
    ('SN-1', 'SUP-PINTAIL', 'Restock teal decoys'),
    ('SN-2', NULL, NULL) AS v(note_id, supplier_code, note_text)
),

__expected__stg_edge_supplier_notes AS (
  SELECT
    NULL AS note_id,
    NULL AS supplier_code,
    NULL AS note_upper
)
