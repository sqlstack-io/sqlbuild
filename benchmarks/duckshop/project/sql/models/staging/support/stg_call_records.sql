MODEL (
  description "One row per live phone call with its IVR menu path",
  columns (
    call_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR),
    agent_id (type VARCHAR),
    caller_phone (type VARCHAR),
    caller_country_prefix (type VARCHAR),
    started_at (type TIMESTAMP_TZ),
    duration_seconds (type INT),
    ivr_path (type VARCHAR),
    ivr_depth (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__call_records")
),

@sql.cdc_latest(src, src.key := (call_id))

final AS (
  SELECT
    c.call_id,
    c.ticket_id,
    c.agent_id,
    @sql.normalise_phone(c.phone_number) AS caller_phone,
    REGEXP_SUBSTR(
      @sql.normalise_phone(c.phone_number),
      '^\\+(1|44|49|33|31|61|81|965)',
      1,
      1,
      'e',
      1
    )::VARCHAR AS caller_country_prefix,
    c.started_at,
    c.duration_seconds::INT AS duration_seconds,
    ARRAY_TO_STRING(c.ivr_path::ARRAY, ' > ')::VARCHAR AS ivr_path,
    ARRAY_SIZE(c.ivr_path)::INT AS ivr_depth,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
