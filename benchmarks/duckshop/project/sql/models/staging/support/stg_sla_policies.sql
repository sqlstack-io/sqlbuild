MODEL (
  description "One row per live support SLA policy for a priority and channel",
  columns (
    policy_id (type VARCHAR, nullable false, audits [not_null, unique]),
    priority (type INT, audits [not_null]),
    channel (type VARCHAR, audits [not_null]),
    first_response_minutes (type INT),
    resolution_minutes (type INT),
    business_hours_only (type BOOLEAN),
    calendar_region_code (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__sla_policies")
),

@sql.cdc_latest(src, src.key := (policy_id))

final AS (
  SELECT
    p.policy_id,
    p.priority::INT AS priority,
    LOWER(p.channel)::VARCHAR AS channel,
    p.first_response_minutes::INT AS first_response_minutes,
    (
      p.resolution_hours * 60
    )::INT AS resolution_minutes,
    COALESCE(p.business_hours_only, TRUE) AS business_hours_only,
    UPPER(COALESCE(p.calendar:region::VARCHAR, 'EU'))::VARCHAR AS calendar_region_code,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
