MODEL (
  description """
    One row per web session: a visitor's events split wherever 30 minutes pass without activity,
    built in daily microbatches from the hourly event stream, with landing page, duration, depth and
    the visitor's local start time from the tracker's reported time zone. Each batch keeps only the
    sessions that start inside it, reading a day of events either side so that sessions crossing
    the batch edges are whole; sessions are assumed to last less than a day. The three-day
    lookback rebuilds recent days when late events arrive.
  """,
  constants (
    _read_margin_hours 24,
  ),
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor session_day,
  cursor_type timestamp,
  cursor_grain day,
  batch_size 1d,
  lookback 3d,
  cursor_inputs (
    stg_web_events (column received_at, roles [watermark]),
  ),
  replay_on_change full,
  on_schema_change sync_all_columns,
  columns (
    session_id (type VARCHAR, nullable false, audits [not_null (run_scope delta_and_final), unique]),
    anonymous_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    session_day (type TIMESTAMP_NTZ, audits [not_null]),
    session_start_utc (type TIMESTAMP_NTZ),
    session_end_utc (type TIMESTAMP_NTZ),
    session_start_visitor_local (type TIMESTAMP_NTZ),
    duration_seconds (type INT),
    event_count (type INT),
    page_view_count (type INT),
    landing_page_path (type VARCHAR),
    exit_page_path (type VARCHAR),
    referrer_host (type VARCHAR),
    landing_page_url (type VARCHAR),
    is_bounce (type BOOLEAN),
    has_add_to_cart (type BOOLEAN),
    has_order (type BOOLEAN),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

human_events AS (
  SELECT
    e.event_id,
    e.anonymous_id,
    e.customer_id,
    e.event_type,
    e.event_at_utc,
    e.page_url,
    e.page_path,
    e.referrer_host,
    e.client_timezone
  FROM web_events AS e
  WHERE
    e.event_at_utc >= DATEADD('hour', -@const("_read_margin_hours"), __cursor_start())
    AND e.event_at_utc < DATEADD('hour', @const("_read_margin_hours"), __cursor_end())
    AND NOT e.is_bot
    AND e.anonymous_id IS NOT NULL
),

gaps AS (
  SELECT
    h.event_id,
    h.anonymous_id,
    h.customer_id,
    h.event_type,
    h.event_at_utc,
    h.page_url,
    h.page_path,
    h.referrer_host,
    h.client_timezone,
    IFF(DATEDIFF(
      'minute',
      LAG(h.event_at_utc) OVER (PARTITION BY h.anonymous_id ORDER BY h.event_at_utc, h.event_id),
      h.event_at_utc
    ) <= @const("session_gap_minutes"), 0, 1) AS is_session_start
  FROM human_events AS h
),

numbered AS (
  SELECT
    g.anonymous_id,
    g.customer_id,
    g.event_type,
    g.event_at_utc,
    g.page_url,
    g.page_path,
    g.referrer_host,
    g.client_timezone,
    SUM(g.is_session_start) OVER (
      PARTITION BY g.anonymous_id
      ORDER BY g.event_at_utc, g.event_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS session_number
  FROM gaps AS g
),

sessions AS (
  SELECT
    n.anonymous_id,
    MAX(n.customer_id) AS customer_id,
    MIN(n.event_at_utc) AS session_start_utc,
    MAX(n.event_at_utc) AS session_end_utc,
    COUNT(*) AS event_count,
    COUNT_IF(n.event_type = 'page_view') AS page_view_count,
    MIN_BY(n.page_path, n.event_at_utc) AS landing_page_path,
    MAX_BY(n.page_path, n.event_at_utc) AS exit_page_path,
    MIN_BY(n.referrer_host, n.event_at_utc) AS referrer_host,
    MIN_BY(n.page_url, n.event_at_utc) AS landing_page_url,
    MIN_BY(n.client_timezone, n.event_at_utc) AS client_timezone,
    BOOLOR_AGG(n.event_type = 'add_to_cart') AS has_add_to_cart,
    BOOLOR_AGG(n.event_type = 'order_completed') AS has_order
  FROM numbered AS n
  GROUP BY
    n.anonymous_id,
    n.session_number
),

final AS (
  SELECT
    (
      s.anonymous_id || ':' || TO_CHAR(s.session_start_utc, 'YYYYMMDDHH24MISS')
    )::VARCHAR AS session_id,
    s.anonymous_id,
    s.customer_id::VARCHAR AS customer_id,
    DATE_TRUNC('day', s.session_start_utc)::TIMESTAMP_NTZ AS session_day,
    s.session_start_utc,
    s.session_end_utc,
    CONVERT_TIMEZONE(
      'UTC',
      COALESCE(s.client_timezone, 'UTC'),
      s.session_start_utc
    )::TIMESTAMP_NTZ AS session_start_visitor_local,
    DATEDIFF('second', s.session_start_utc, s.session_end_utc)::INT AS duration_seconds,
    s.event_count::INT AS event_count,
    s.page_view_count::INT AS page_view_count,
    s.landing_page_path::VARCHAR AS landing_page_path,
    s.exit_page_path::VARCHAR AS exit_page_path,
    s.referrer_host::VARCHAR AS referrer_host,
    s.landing_page_url::VARCHAR AS landing_page_url,
    (
      s.page_view_count <= 1
      AND s.event_count <= 1
    ) AS is_bounce,
    s.has_add_to_cart::BOOLEAN AS has_add_to_cart,
    s.has_order::BOOLEAN AS has_order
  FROM sessions AS s
  WHERE
    s.session_start_utc >= __cursor_start()
    AND s.session_start_utc < __cursor_end()
)

SELECT *
FROM final
