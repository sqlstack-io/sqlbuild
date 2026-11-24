MODEL (
  description "One row per push notification opened in the app, with the campaign it belongs to",
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    device_id (type VARCHAR),
    customer_id (type VARCHAR),
    opened_at_utc (type TIMESTAMP_NTZ),
    push_campaign_id (type VARCHAR),
    deep_link_path (type VARCHAR),
  ),
);

WITH app_events AS (
  SELECT *
  FROM __ref("stg_app_events")
),

final AS (
  SELECT
    e.event_id,
    e.device_id,
    e.customer_id,
    e.event_at_utc AS opened_at_utc,
    COALESCE(e.attributes:campaign_id::VARCHAR, REGEXP_SUBSTR(
      e.attributes:deep_link::VARCHAR,
      'cid=([A-Za-z0-9-]+)',
      1,
      1,
      'e',
      1
    ))::VARCHAR AS push_campaign_id,
    REGEXP_REPLACE(
      e.attributes:deep_link::VARCHAR,
      '^[a-z]+://[^/]*',
      ''
    )::VARCHAR AS deep_link_path
  FROM app_events AS e
  WHERE e.event_name = 'push_open'
)

SELECT *
FROM final
