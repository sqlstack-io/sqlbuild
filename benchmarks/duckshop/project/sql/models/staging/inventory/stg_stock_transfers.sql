MODEL (
  description "One row per live inter-warehouse stock transfer",
  columns (
    transfer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    from_warehouse_code (type VARCHAR),
    to_warehouse_code (type VARCHAR),
    qty (type INT),
    shipped_at (type TIMESTAMP_TZ),
    arrived_at (type TIMESTAMP_TZ),
    transit_hours (type INT),
    haulier_reference (type VARCHAR),
    tracking (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__stock_transfers")
),

@sql.cdc_latest(src, src.key := (transfer_id))

final AS (
  SELECT
    t.transfer_id,
    @sql.normalise_sku(t.sku) AS sku,
    UPPER(t.from_warehouse_code)::VARCHAR AS from_warehouse_code,
    UPPER(t.to_warehouse_code)::VARCHAR AS to_warehouse_code,
    t.qty::INT AS qty,
    t.shipped_at,
    t.arrived_at,
    DATEDIFF('hour', t.shipped_at, t.arrived_at)::INT AS transit_hours,
    REGEXP_SUBSTR(
      t.tracking:reference::VARCHAR,
      '[A-Z]{2}[0-9]{6,}'
    )::VARCHAR AS haulier_reference,
    t.tracking::OBJECT AS tracking,
    @sql.load_metadata(t)
  FROM cdc_latest AS t
)
SELECT *
FROM final
