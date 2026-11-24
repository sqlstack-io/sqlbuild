MODEL (
  description """
    One row per web session with its attributed channel and campaign, engagement and conversion
    flags. Merged incrementally from the daily sessionised clickstream with a two-day lookback for
    sessions that span midnight. The visitor bucket hashes the anonymous visitor id with HASH, whose
    output is engine-specific, so buckets are an expected divergence across engines.
  """,
  tags [known_divergence],
  materialized incremental,
  incremental_strategy merge,
  unique_key [session_id],
  cursor session_start_utc,
  cursor_type timestamp,
  cursor_grain hour,
  lookback 2d,
  cursor_inputs (
    int_web_sessions session_start_utc,
  ),
  replay_on_change bounded-120d,
  on_schema_change append_new_columns,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    session_id (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    session_start_utc (type TIMESTAMP_NTZ, nullable false, audits [not_null]),
    session_start_visitor_local (type TIMESTAMP_NTZ),
    duration_seconds (type INT),
    page_view_count (type INT),
    landing_page_path (type VARCHAR),
    channel_code (type VARCHAR),
    is_paid (type BOOLEAN),
    utm_campaign (type VARCHAR),
    first_touch_channel (type VARCHAR),
    touch_number (type INT),
    is_bounce (type BOOLEAN),
    funnel_stage (type VARCHAR),
    converted (type BOOLEAN),
    visitor_bucket (type INT),
  ),
);

WITH sessions AS (
  SELECT *
  FROM __ref("int_web_sessions")
),

attribution AS (
  SELECT *
  FROM __ref("int_session_attribution")
),

funnels AS (
  SELECT *
  FROM __ref("int_cart_funnel")
),

windowed AS (
  SELECT
    s.session_id,
    s.anonymous_id,
    s.customer_id,
    s.session_start_utc,
    s.session_start_visitor_local,
    s.duration_seconds,
    s.page_view_count,
    s.landing_page_path,
    s.is_bounce,
    s.has_order
  FROM sessions AS s
  WHERE
    s.session_start_utc >= __cursor_start()
    AND s.session_start_utc < __cursor_end()
),

final AS (
  SELECT
    w.session_id,
    w.anonymous_id,
    w.customer_id,
    w.session_start_utc,
    w.session_start_visitor_local,
    w.duration_seconds,
    w.page_view_count,
    w.landing_page_path,
    a.channel_code,
    a.is_paid,
    a.utm_campaign,
    a.first_touch_channel,
    a.touch_number,
    w.is_bounce,
    COALESCE(f.funnel_stage, IFF(w.is_bounce, 'bounce', 'browse'))::VARCHAR AS funnel_stage,
    COALESCE(f.converted, w.has_order, FALSE)::BOOLEAN AS converted,
    MOD(ABS(HASH(w.anonymous_id)), 100)::INT AS visitor_bucket
  FROM windowed AS w
  LEFT JOIN attribution AS a
    ON w.session_id = a.session_id
  LEFT JOIN funnels AS f
    ON w.session_id = f.session_id
)

SELECT *
FROM final
