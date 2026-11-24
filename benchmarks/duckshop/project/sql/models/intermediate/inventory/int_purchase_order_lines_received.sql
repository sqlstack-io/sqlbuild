MODEL (
  description """
    One row per purchase order line with the quantity received, damaged and still outstanding, the
    fill rate, and how late the first receipt was against the expected date.
  """,
  columns (
    po_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    po_id (type VARCHAR, audits [not_null]),
    supplier_code (type VARCHAR),
    warehouse_code (type VARCHAR),
    sku (type VARCHAR),
    ordered_qty (type INT),
    received_qty (type INT),
    damaged_qty (type INT),
    outstanding_qty (type INT),
    fill_rate (type NUMBER(9, 4)),
    first_received_at_local (type TIMESTAMP_NTZ),
    days_late (type INT),
    receipt_count (type INT),
    discrepancy_codes (type ARRAY),
  ),
);

WITH po_lines AS (
  SELECT *
  FROM __ref("stg_purchase_order_lines")
),

purchase_orders AS (
  SELECT *
  FROM __ref("stg_purchase_orders")
),

receipts AS (
  SELECT *
  FROM __ref("stg_po_receipts")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

receipt_totals AS (
  SELECT
    r.po_id,
    r.sku,
    SUM(r.received_qty) AS received_qty,
    SUM(r.damaged_qty) AS damaged_qty,
    MIN(r.received_at_local) AS first_received_at_local,
    COUNT(*) AS receipt_count,
    ARRAY_DISTINCT(ARRAY_FLATTEN(ARRAY_AGG(r.discrepancy_codes))) AS discrepancy_codes
  FROM receipts AS r
  GROUP BY
    r.po_id,
    r.sku
),

expected_local AS (
  SELECT
    p.po_id,
    p.supplier_code,
    p.warehouse_code,
    CONVERT_TIMEZONE(w.iana_timezone, p.expected_at)::TIMESTAMP_NTZ AS expected_at_local
  FROM purchase_orders AS p
  INNER JOIN warehouses AS w
    ON p.warehouse_code = w.warehouse_code
),

final AS (
  SELECT
    l.po_line_key,
    l.po_id,
    e.supplier_code,
    e.warehouse_code,
    l.sku,
    l.ordered_qty,
    COALESCE(r.received_qty, 0)::INT AS received_qty,
    COALESCE(r.damaged_qty, 0)::INT AS damaged_qty,
    GREATEST(l.ordered_qty - COALESCE(r.received_qty, 0), 0)::INT AS outstanding_qty,
    ROUND(
      LEAST(COALESCE(r.received_qty, 0) / NULLIF(l.ordered_qty, 0), 1),
      4
    )::NUMBER(9, 4) AS fill_rate,
    r.first_received_at_local,
    GREATEST(
      DATEDIFF('day', e.expected_at_local, r.first_received_at_local),
      0
    )::INT AS days_late,
    COALESCE(r.receipt_count, 0)::INT AS receipt_count,
    COALESCE(r.discrepancy_codes, ARRAY_CONSTRUCT())::ARRAY AS discrepancy_codes
  FROM po_lines AS l
  INNER JOIN expected_local AS e
    ON l.po_id = e.po_id
  LEFT JOIN receipt_totals AS r
    ON l.po_id = r.po_id
    AND l.sku = r.sku
)

SELECT *
FROM final
