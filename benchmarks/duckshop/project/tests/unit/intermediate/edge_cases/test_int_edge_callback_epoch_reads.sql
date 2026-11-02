TEST (name "int_edge_callback_epoch_reads__reads_keys_as_epoch_milliseconds");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id,
    v.group_code::VARCHAR AS group_code,
    v.narrow_key::NUMBER(18, 0) AS narrow_key,
    v.wide_key::NUMBER(38, 0) AS wide_key,
    v.key_text::VARCHAR AS key_text,
    v.amount::NUMBER(18, 2) AS amount,
    v.ratio::FLOAT AS ratio
  FROM VALUES
    ('AC-01', 'G-1', 7, 7, '101,7', 10.05, 0.1),
    ('AC-02', 'G-1', 12, 12, '2026-03-04', -0.55, 0.2),
    ('AC-03', 'G-2', 3, NULL, '7', 0.00, 0.3),
    ('AC-04', 'G-2', NULL, 40, NULL, NULL, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, ratio)
),

__ref__stg_edge_gateway_callbacks AS (
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

__expected__int_edge_callback_epoch_reads AS (
  SELECT
    'AC-01' AS check_id,
    '1970-01-01 00:00:00.007'::TIMESTAMP_NTZ(9) AS key_as_time,
    TRUE AS has_early_callback
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    '1970-01-01 00:00:00.012'::TIMESTAMP_NTZ(9) AS key_as_time,
    TRUE AS has_early_callback
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    CAST(NULL AS TIMESTAMP_NTZ(9)) AS key_as_time,
    TRUE AS has_early_callback
  UNION ALL
  SELECT
    'AC-04' AS check_id,
    '1970-01-01 00:00:00.04'::TIMESTAMP_NTZ(9) AS key_as_time,
    TRUE AS has_early_callback
)
