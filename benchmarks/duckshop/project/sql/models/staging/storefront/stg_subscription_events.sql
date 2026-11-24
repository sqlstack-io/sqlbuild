MODEL (
  description "One row per live subscription lifecycle event",
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    subscription_id (type VARCHAR, audits [not_null]),
    event_type (type VARCHAR, audits [not_null]),
    occurred_at (type TIMESTAMP_TZ, audits [not_null]),
    pause_days (type INT),
    skip_reason (type VARCHAR),
    payload (type VARIANT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__subscription_events")
),

@sql.cdc_latest(src, src.key := (event_id))

final AS (
  SELECT
    e.event_id,
    e.subscription_id,
    UPPER(e.event_type)::VARCHAR AS event_type,
    e.occurred_at,
    TRY_TO_NUMBER(e.payload:pause_days::VARCHAR)::INT AS pause_days,
    @sql.json_text(e.payload:skip_reason) AS skip_reason,
    e.payload::VARIANT AS payload,
    @sql.load_metadata(e)
  FROM cdc_latest AS e
)
SELECT *
FROM final
