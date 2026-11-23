MODEL (
  description "One row per live purchase order header",
  columns (
    po_id (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_code (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    po_status (type VARCHAR),
    ordered_at (type TIMESTAMP_TZ),
    expected_at (type TIMESTAMP_TZ),
    received_at (type TIMESTAMP_TZ),
    line_count (type INT),
    lines (type ARRAY),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__purchase_orders")
),

@sql.cdc_latest(src, src.key := (po_id))

final AS (
  SELECT
    p.po_id,
    UPPER(p.supplier_code)::VARCHAR AS supplier_code,
    UPPER(p.warehouse_code)::VARCHAR AS warehouse_code,
    UPPER(p.currency_code)::VARCHAR AS currency_code,
    LOWER(p.po_status)::VARCHAR AS po_status,
    p.ordered_at,
    p.expected_at,
    p.received_at,
    ARRAY_SIZE(COALESCE(p.lines, ARRAY_CONSTRUCT()))::INT AS line_count,
    p.lines::ARRAY AS lines,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
