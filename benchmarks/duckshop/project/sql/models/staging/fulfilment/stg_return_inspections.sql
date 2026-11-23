MODEL (
  description "One row per live returned-item inspection in warehouse local time",
  columns (
    inspection_id (type VARCHAR, nullable false, audits [not_null, unique]),
    return_id (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    grade (type VARCHAR, audits [accepted_values (values ["A", "B", "C", "SCRAP"])]),
    is_resellable (type BOOLEAN),
    inspected_at_local (type TIMESTAMP_NTZ),
    defect_codes (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__return_inspections")
),

@sql.cdc_latest(src, src.key := (inspection_id))

final AS (
  SELECT
    i.inspection_id,
    i.return_id,
    UPPER(i.warehouse_code)::VARCHAR AS warehouse_code,
    UPPER(TRIM(i.grade))::VARCHAR AS grade,
    (
      UPPER(TRIM(i.grade)) IN ('A', 'B')
    ) AS is_resellable,
    i.inspected_at_local,
    COALESCE(i.findings:defects, ARRAY_CONSTRUCT())::ARRAY AS defect_codes,
    @sql.load_metadata(i)
  FROM cdc_latest AS i
)
SELECT *
FROM final
