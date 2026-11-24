MODEL (
  description "One row per live order status transition with the acting system or person",
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    from_status (type VARCHAR),
    to_status (type VARCHAR, audits [not_null]),
    changed_at (type TIMESTAMP_TZ, audits [not_null]),
    actor_type (type VARCHAR),
    actor_id (type VARCHAR),
    actor (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__order_status_events")
),

@sql.cdc_latest(src, src.key := (event_id))

final AS (
  SELECT
    e.event_id,
    e.order_id,
    LOWER(e.from_status)::VARCHAR AS from_status,
    LOWER(e.to_status)::VARCHAR AS to_status,
    e.changed_at,
    COALESCE(
      e.actor:type::VARCHAR,
      IFF(e.actor IS NULL, 'system', 'unknown')
    )::VARCHAR AS actor_type,
    @sql.json_text(e.actor:id) AS actor_id,
    e.actor::OBJECT AS actor,
    @sql.load_metadata(e)
  FROM cdc_latest AS e
)
SELECT *
FROM final
