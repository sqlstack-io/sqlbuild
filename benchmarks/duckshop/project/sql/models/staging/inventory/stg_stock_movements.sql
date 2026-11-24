MODEL (
  description """
    One row per effective stock movement in warehouse local time. The append-only log records
    corrections as VOID rows referencing the movement they cancel; voided and void rows are removed.
  """,
  columns (
    movement_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    qty_delta (type INT, audits [not_null]),
    reason_code (type VARCHAR),
    reference_type (type VARCHAR),
    reference_id (type VARCHAR),
    moved_at_local (type TIMESTAMP_NTZ, audits [not_null]),
    operator_id (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__stock_movements")
),

deduplicated AS (
  SELECT
    m.movement_id,
    m.sku,
    m.warehouse_code,
    m.qty_delta,
    UPPER(m.reason_code) AS reason_code,
    m.reference,
    m.moved_at_local,
    m.metadata
  FROM src AS m
  QUALIFY ROW_NUMBER() OVER (PARTITION BY m.movement_id ORDER BY m._loaded_at DESC) = 1
),

effective_ids AS (
  SELECT d.movement_id
  FROM deduplicated AS d
  WHERE d.reason_code <> 'VOID'
  MINUS
  SELECT SPLIT_PART(v.reference, ':', 2) AS movement_id
  FROM deduplicated AS v
  WHERE v.reason_code = 'VOID'
),

final AS (
  SELECT
    d.movement_id,
    @sql.normalise_sku(d.sku) AS sku,
    UPPER(d.warehouse_code)::VARCHAR AS warehouse_code,
    d.qty_delta::INT AS qty_delta,
    d.reason_code::VARCHAR AS reason_code,
    UPPER(SPLIT_PART(d.reference, ':', 1))::VARCHAR AS reference_type,
    NULLIF(SPLIT_PART(d.reference, ':', 2), '')::VARCHAR AS reference_id,
    d.moved_at_local,
    @sql.json_text(d.metadata:operator) AS operator_id
  FROM deduplicated AS d
  INNER JOIN effective_ids AS e
    ON d.movement_id = e.movement_id
)

SELECT *
FROM final
