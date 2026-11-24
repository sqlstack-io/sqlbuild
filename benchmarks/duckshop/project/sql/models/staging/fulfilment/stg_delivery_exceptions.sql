MODEL (
  description "One row per live delivery exception with its resolution",
  columns (
    exception_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    exception_code (type VARCHAR),
    exception_text (type VARCHAR),
    occurred_at (type TIMESTAMP_TZ),
    resolution_type (type VARCHAR),
    resolved_at (type TIMESTAMP_TZ),
    compensation_minor (type INT),
    resolution (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__delivery_exceptions")
),

@sql.cdc_latest(src, src.key := (exception_id))

final AS (
  SELECT
    e.exception_id,
    e.shipment_id,
    UPPER(e.exception_code)::VARCHAR AS exception_code,
    TRIM(e.exception_text)::VARCHAR AS exception_text,
    e.occurred_at,
    LOWER(@sql.json_text(e.resolution:type))::VARCHAR AS resolution_type,
    TRY_TO_TIMESTAMP_TZ(e.resolution:at::VARCHAR) AS resolved_at,
    COALESCE(e.resolution:compensation_minor::INT, 0)::INT AS compensation_minor,
    e.resolution::OBJECT AS resolution,
    @sql.load_metadata(e)
  FROM cdc_latest AS e
)
SELECT *
FROM final
