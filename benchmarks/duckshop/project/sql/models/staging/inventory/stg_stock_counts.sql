MODEL (
  description "One row per live cycle count in warehouse local time",
  columns (
    count_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    counted_qty (type INT),
    counted_at_local (type TIMESTAMP_NTZ, audits [not_null]),
    counter_id (type VARCHAR),
    is_blind_count (type BOOLEAN),
    counter (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__stock_counts")
),

@sql.cdc_latest(src, src.key := (count_id))

final AS (
  SELECT
    c.count_id,
    @sql.normalise_sku(c.sku) AS sku,
    UPPER(c.warehouse_code)::VARCHAR AS warehouse_code,
    GREATEST(c.counted_qty, 0)::INT AS counted_qty,
    c.counted_at_local,
    @sql.json_text(c.counter:id) AS counter_id,
    COALESCE(c.counter:blind::BOOLEAN, FALSE) AS is_blind_count,
    c.counter::OBJECT AS counter,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
