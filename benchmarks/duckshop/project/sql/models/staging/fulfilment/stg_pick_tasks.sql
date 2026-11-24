MODEL (
  description "One row per live warehouse pick task in warehouse local time",
  columns (
    task_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    picker_id (type VARCHAR),
    started_at_local (type TIMESTAMP_NTZ),
    completed_at_local (type TIMESTAMP_NTZ),
    pick_seconds (type INT),
    line_count (type INT),
    units_picked (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__pick_tasks")
),

@sql.cdc_latest(src, src.key := (task_id))

final AS (
  SELECT
    t.task_id,
    t.order_id,
    UPPER(t.warehouse_code)::VARCHAR AS warehouse_code,
    t.picker_id,
    t.started_at_local,
    t.completed_at_local,
    DATEDIFF('second', t.started_at_local, t.completed_at_local)::INT AS pick_seconds,
    ARRAY_SIZE(t.lines)::INT AS line_count,
    REDUCE(
      t.lines::ARRAY,
      0,
      (total, l) -> total + COALESCE(l:qty::INT, 0)
    )::INT AS units_picked,
    @sql.load_metadata(t)
  FROM cdc_latest AS t
)
SELECT *
FROM final
