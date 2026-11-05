TEST (name "int_edge_ledger_quantity_steps__subtracts_one_from_quantity_text");

WITH __ref__stg_edge_ledger_entries AS (
  SELECT
    v.entry_id::VARCHAR AS entry_id,
    v.order_ref::VARCHAR AS order_ref,
    v.quantity_text::VARCHAR AS quantity_text,
    v.status::VARCHAR AS status
  FROM VALUES
    ('LE-01', 'DS-O-1', '2', 'paid'),
    ('LE-02', 'DS-O-1', '3.5', 'open'),
    ('LE-03', 'DS-O-2', NULL, 'paid') AS v(entry_id, order_ref, quantity_text, status)
),

__expected__int_edge_ledger_quantity_steps AS (
  SELECT
    'DS-O-1' AS order_ref,
    2.500000::NUMBER(38,6) AS quantity_less_one
  UNION ALL
  SELECT
    'DS-O-2' AS order_ref,
    CAST(NULL AS NUMBER(38,6)) AS quantity_less_one
)
