MODEL (
  description "One row per app screen view, with time spent until the next event on the device",
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    device_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    platform (type VARCHAR),
    screen_name (type VARCHAR),
    viewed_at_utc (type TIMESTAMP_NTZ),
    seconds_on_screen (type INT),
    screen_class (type VARCHAR),
  ),
);

WITH app_events AS (
  SELECT *
  FROM __ref("stg_app_events")
),

ordered AS (
  SELECT
    e.event_id,
    e.device_id,
    e.customer_id,
    e.platform,
    e.event_name,
    e.screen_name,
    e.event_at_utc,
    e.attributes:screen_class::VARCHAR AS screen_class,
    LEAD(e.event_at_utc) OVER (
      PARTITION BY e.device_id
      ORDER BY e.event_at_utc, e.event_id
    ) AS next_event_at_utc
  FROM app_events AS e
  WHERE e.device_id IS NOT NULL
),

final AS (
  SELECT
    o.event_id,
    o.device_id,
    o.customer_id,
    o.platform,
    INITCAP(REPLACE(o.screen_name, '_', ' '))::VARCHAR AS screen_name,
    o.event_at_utc AS viewed_at_utc,
    LEAST(DATEDIFF('second', o.event_at_utc, o.next_event_at_utc), 1800)::INT AS seconds_on_screen,
    o.screen_class::VARCHAR AS screen_class
  FROM ordered AS o
  WHERE o.event_name = 'screen_view'
)

SELECT *
FROM final
