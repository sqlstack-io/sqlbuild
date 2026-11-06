TEST (name "int_edge_callback_recent_offsets__keeps_callbacks_from_march_by_their_zoned_time");

WITH __ref__stg_edge_gateway_callbacks AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id,
    v.merchant_zone::VARCHAR AS merchant_zone,
    v.local_wall_time::TIMESTAMP_NTZ AS local_wall_time,
    v.event_at::TIMESTAMP_TZ AS event_at,
    v.event_at_ns::TIMESTAMP_NTZ(9) AS event_at_ns,
    v.event_time_text::VARCHAR AS event_time_text,
    v.business_date::DATE AS business_date
  FROM VALUES
    ('GC-01', 'Europe/London', '2026-03-28 23:30:00', '2026-03-28 23:30:00 +00:00', '2026-03-28 23:30:00.123456789', '2026-03-28T23:30:00+00:00', '2026-03-28'),
    ('GC-02', 'America/New_York', '2026-03-08 01:15:00', '2026-03-08 01:15:00 -05:00', '2026-03-08 01:15:00.5', '2026-03-08T01:15:00-05:00', '2026-03-08'),
    ('GC-03', 'Asia/Kolkata', '2026-02-28 18:45:30', '2026-02-28 18:45:30 +05:30', NULL, '2026-02-28', '2026-02-28') AS v(callback_id, merchant_zone, local_wall_time, event_at, event_at_ns, event_time_text, business_date)
),

__expected__int_edge_callback_recent_offsets AS (
  SELECT
    'GC-01' AS callback_id,
    '2026-03-28 23:30:00 Z'::TIMESTAMP_TZ(9) AS event_at
  UNION ALL
  SELECT
    'GC-02' AS callback_id,
    '2026-03-08 01:15:00 -05:00'::TIMESTAMP_TZ(9) AS event_at
)
