MODEL (
  description """
    One row per ISO week and platform with app sessions, active devices, push-started share and
    session length
  """,
  columns (
    app_week_key (type VARCHAR, nullable false, audits [not_null, unique]),
    week_start (type DATE),
    platform (type VARCHAR),
    sessions (type INT),
    active_devices (type INT),
    signed_in_share (type NUMBER(9, 4)),
    push_started_share (type NUMBER(9, 4)),
    median_session_seconds (type NUMBER(10, 2)),
    latest_major_version (type INT),
  ),
);

WITH app_sessions AS (
  SELECT *
  FROM __ref("int_app_sessions")
),

final AS (
  SELECT
    (
      TO_VARCHAR(DATE_TRUNC('week', a.session_start_utc), 'YYYY-MM-DD') || ':' || a.platform
    )::VARCHAR AS app_week_key,
    DATE_TRUNC('week', a.session_start_utc)::DATE AS week_start,
    a.platform,
    COUNT(*)::INT AS sessions,
    COUNT(DISTINCT a.device_id)::INT AS active_devices,
    ROUND(AVG(IFF(a.customer_id IS NOT NULL, 1, 0)), 4)::NUMBER(9, 4) AS signed_in_share,
    ROUND(AVG(IFF(a.started_from_push, 1, 0)), 4)::NUMBER(9, 4) AS push_started_share,
    ROUND(MEDIAN(a.duration_seconds), 2)::NUMBER(10, 2) AS median_session_seconds,
    MAX(a.app_major_version)::INT AS latest_major_version
  FROM app_sessions AS a
  GROUP BY
    DATE_TRUNC('week', a.session_start_utc),
    a.platform
)

SELECT *
FROM final
