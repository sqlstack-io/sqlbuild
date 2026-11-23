MODEL (
  description """
    One row per ledger line whose quantity text is numeric, comparing text and numbers with text
    literals, and casting the text only after a filter has removed text that is not a number
  """,
  tags [edge_case],
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    quantity_text (type VARCHAR),
    text_equals_seven (type BOOLEAN),
    quantity_equals_seven_text (type BOOLEAN),
    quantity_cast (type NUMBER(18, 0)),
  ),
);

WITH lines AS (
  SELECT
    e.entry_id,
    e.quantity,
    e.quantity_text
  FROM __ref("stg_edge_ledger_entries") AS e
),

numeric_lines AS (
  SELECT
    l.entry_id,
    l.quantity,
    l.quantity_text
  FROM lines AS l
  WHERE TRY_TO_NUMBER(l.quantity_text) IS NOT NULL
),

final AS (
  SELECT
    n.entry_id,
    n.quantity_text,
    n.quantity_text = '7' AS text_equals_seven,
    n.quantity = '7' AS quantity_equals_seven_text,
    n.quantity_text::NUMBER(18, 0) AS quantity_cast
  FROM numeric_lines AS n
)

SELECT *
FROM final
