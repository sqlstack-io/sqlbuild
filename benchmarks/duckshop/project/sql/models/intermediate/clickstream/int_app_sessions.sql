MODEL (
  description """
    One row per app session, split on 30 minutes of inactivity per device, with screens viewed and
    push-driven starts
  """,
  columns (
    app_session_id (type VARCHAR, nullable false, audits [not_null, unique]),
    device_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    platform (type VARCHAR),
    app_major_version (type INT),
    session_start_utc (type TIMESTAMP_NTZ),
    session_end_utc (type TIMESTAMP_NTZ),
    duration_seconds (type INT),
    event_count (type INT),
    screen_count (type INT),
    started_from_push (type BOOLEAN),
  ),
);

WITH app_events AS (
  SELECT *
  FROM __ref("stg_app_events")
),

device_events AS (
  SELECT
    e.event_id,
    e.device_id,
    e.customer_id,
    e.platform,
    e.app_major_version,
    e.event_name,
    e.event_at_utc,
    IFF(COALESCE(DATEDIFF(
      'minute',
      LAG(e.event_at_utc) OVER (PARTITION BY e.device_id ORDER BY e.event_at_utc, e.event_id),
      e.event_at_utc
    ), 999) > @const("session_gap_minutes"), 1, 0) AS starts_session
  FROM app_events AS e
  WHERE
    e.device_id IS NOT NULL
    AND e.event_at_utc IS NOT NULL
),

numbered AS (
  SELECT
    d.device_id,
    d.customer_id,
    d.platform,
    d.app_major_version,
    d.event_name,
    d.event_at_utc,
    d.starts_session,
    SUM(d.starts_session) OVER (
      PARTITION BY d.device_id
      ORDER BY d.event_at_utc, d.event_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS session_number
  FROM device_events AS d
),

sessions AS (
  SELECT
    n.device_id,
    MAX(n.customer_id) AS customer_id,
    ANY_VALUE(n.platform) AS platform,
    MAX(n.app_major_version) AS app_major_version,
    MIN(n.event_at_utc) AS session_start_utc,
    MAX(n.event_at_utc) AS session_end_utc,
    COUNT(*) AS event_count,
    COUNT_IF(n.event_name = 'screen_view') AS screen_count,
    BOOLOR_AGG(n.starts_session = 1 AND n.event_name = 'push_open') AS started_from_push
  FROM numbered AS n
  GROUP BY
    n.device_id,
    n.session_number
),

final AS (
  SELECT
    (
      s.device_id || ':' || TO_CHAR(s.session_start_utc, 'YYYYMMDDHH24MISS')
    )::VARCHAR AS app_session_id,
    s.device_id,
    s.customer_id::VARCHAR AS customer_id,
    s.platform::VARCHAR AS platform,
    s.app_major_version::INT AS app_major_version,
    s.session_start_utc,
    s.session_end_utc,
    DATEDIFF('second', s.session_start_utc, s.session_end_utc)::INT AS duration_seconds,
    s.event_count::INT AS event_count,
    s.screen_count::INT AS screen_count,
    s.started_from_push::BOOLEAN AS started_from_push
  FROM sessions AS s
)

SELECT *
FROM final
