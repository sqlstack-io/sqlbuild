TEST (name "int_edge_ledger_paid_quantities__lists_paid_quantities_in_change_order");

WITH __ref__stg_edge_ledger_entries AS (
  SELECT
    v.entry_id::VARCHAR AS entry_id,
    v.order_ref::VARCHAR AS order_ref,
    v.quantity_text::VARCHAR AS quantity_text,
    v.status::VARCHAR AS status,
    v.changed_at::TIMESTAMP_NTZ AS changed_at
  FROM VALUES
    ('LE-01', 'DS-O-1', '2', 'paid', '2026-03-01 10:00:00'),
    ('LE-02', 'DS-O-1', '3.5', 'open', '2026-03-01 11:00:00'),
    ('LE-03', 'DS-O-2', NULL, 'paid', '2026-03-02 09:00:00') AS v(entry_id, order_ref, quantity_text, status, changed_at)
),

__expected__int_edge_ledger_paid_quantities AS (
  SELECT
    'DS-O-1' AS order_ref,
    1 AS paid_quantity_count
  UNION ALL
  SELECT
    'DS-O-2' AS order_ref,
    0 AS paid_quantity_count
)
