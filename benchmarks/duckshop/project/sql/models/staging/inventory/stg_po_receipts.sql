MODEL (
  description "One row per live purchase order receipt in warehouse local time",
  columns (
    receipt_id (type VARCHAR, nullable false, audits [not_null, unique]),
    po_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    received_qty (type INT),
    damaged_qty (type INT),
    sellable_qty (type INT),
    received_at_local (type TIMESTAMP_NTZ),
    discrepancy_codes (type ARRAY),
    has_discrepancy (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__po_receipts")
),

@sql.cdc_latest(src, src.key := (receipt_id))

final AS (
  SELECT
    r.receipt_id,
    r.po_id,
    @sql.normalise_sku(r.sku) AS sku,
    UPPER(r.warehouse_code)::VARCHAR AS warehouse_code,
    r.received_qty::INT AS received_qty,
    COALESCE(r.damaged_qty, 0)::INT AS damaged_qty,
    (
      r.received_qty - COALESCE(r.damaged_qty, 0)
    )::INT AS sellable_qty,
    r.received_at_local,
    COALESCE(r.discrepancies:codes, ARRAY_CONSTRUCT())::ARRAY AS discrepancy_codes,
    (
      ARRAY_SIZE(COALESCE(r.discrepancies:codes, ARRAY_CONSTRUCT())) > 0
    ) AS has_discrepancy,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
