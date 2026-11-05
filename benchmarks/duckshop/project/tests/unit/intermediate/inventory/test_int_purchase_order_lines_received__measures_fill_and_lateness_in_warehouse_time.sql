TEST (name "int_purchase_order_lines_received__measures_fill_and_lateness_in_warehouse_time");

WITH __ref__stg_po_receipts AS (
  SELECT
    'R1' AS receipt_id,
    'PO1' AS po_id,
    'DUCK-1' AS sku,
    'W1' AS warehouse_code,
    6 AS received_qty,
    1 AS damaged_qty,
    NULL AS sellable_qty,
    '2026-03-12 10:00:00'::TIMESTAMP_NTZ AS received_at_local,
    ARRAY_CONSTRUCT('SHORT')::ARRAY AS discrepancy_codes,
    NULL AS has_discrepancy,
    NULL AS source_seq,
    NULL AS source_loaded_at
  UNION ALL
  SELECT
    'R2',
    'PO1',
    'DUCK-1',
    'W1',
    6,
    0,
    NULL,
    '2026-03-13 10:00:00'::TIMESTAMP_NTZ,
    ARRAY_CONSTRUCT('SHORT', 'DAMAGED')::ARRAY,
    NULL,
    NULL,
    NULL
),

__ref__stg_purchase_order_lines AS (
  SELECT
    'PO1#1' AS po_line_key,
    'PO1' AS po_id,
    'DUCK-1' AS sku,
    10 AS ordered_qty,
    NULL AS unit_cost_minor,
    NULL AS currency_code
  UNION ALL
  SELECT
    'PO1#2',
    'PO1',
    'DUCK-2',
    5,
    NULL,
    NULL
),

__ref__stg_purchase_orders AS (
  SELECT
    'PO1' AS po_id,
    'SUP1' AS supplier_code,
    'W1' AS warehouse_code,
    NULL AS currency_code,
    NULL AS po_status,
    NULL AS ordered_at,
    '2026-03-10 23:30:00 +00:00'::TIMESTAMP_TZ AS expected_at,
    NULL AS received_at,
    NULL AS line_count,
    NULL AS lines,
    NULL AS source_seq,
    NULL AS source_loaded_at
),

__ref__stg_warehouses AS (
  SELECT
    'W1' AS warehouse_code,
    NULL AS warehouse_name,
    NULL AS country_code,
    'Europe/Berlin' AS iana_timezone,
    NULL AS opens_local,
    NULL AS closes_local,
    NULL AS opening_minutes,
    NULL AS supports_same_day,
    NULL AS supports_cold_chain,
    NULL AS max_parcel_kg,
    NULL AS source_seq,
    NULL AS source_loaded_at
),

__expected__int_purchase_order_lines_received AS (
  SELECT
    'PO1#1' AS po_line_key,
    12 AS received_qty,
    1 AS damaged_qty,
    0 AS outstanding_qty,
    1.0::DECIMAL(9, 4) AS fill_rate,
    1 AS days_late,
    2 AS receipt_count
  UNION ALL
  SELECT
    'PO1#2' AS po_line_key,
    0 AS received_qty,
    0 AS damaged_qty,
    5 AS outstanding_qty,
    0.0::DECIMAL(9, 4) AS fill_rate,
    CAST(NULL AS INT) AS days_late,
    0 AS receipt_count
),

__assert__discrepancy_codes_are_deduplicated AS (
  SELECT l.po_line_key
  FROM __ref("int_purchase_order_lines_received") AS l
  WHERE
    l.po_line_key = 'PO1#1'
    AND ARRAY_SIZE(l.discrepancy_codes) <> 2
)
