TEST (name "int_edge_clock_text_reads__reads_clock_text_parts_and_clock_free_facts");

WITH __ref__stg_edge_date_texts AS (
  SELECT
    v.text_id,
    NULL::VARCHAR AS date_text,
    v.zoned_text,
    NULL::TIMESTAMP_NTZ(3) AS wall_clock,
    v.wall_clock_text,
    NULL::TIME(3) AS local_time,
    NULL::TIME(9) AS fine_time
  FROM VALUES
    ('CT-01', '2026-03-06T08:30:00+09:00', '2026-03-05 23:30:00.1239'),
    ('CT-02', '2026-03-05T23:30:00Z', '1969-12-31 23:59:59.999'),
    ('CT-03', '2026-12-31T23:30:00-05:30', '2026-12-31 00:00:00'),
    ('CT-04', NULL, NULL) AS v(text_id, zoned_text, wall_clock_text)
),

__expected__int_edge_clock_text_reads AS (
  SELECT
    'CT-01' AS text_id,
    '2026-03-05'::DATE AS clock_date,
    '23:30:00.1239'::TIME(9) AS clock_time,
    '2026-03-05 23:30:00.1239'::TIMESTAMP_NTZ(9) AS clock_timestamp,
    '2026-03-05 23:30:00'::TIMESTAMP_NTZ(9) AS zoned_instant_utc,
    TRUE AS today_is_after_2020,
    0 AS today_minus_today,
    TRUE AS system_clock_is_after_2020
  UNION ALL
  SELECT
    'CT-02' AS text_id,
    '1969-12-31'::DATE AS clock_date,
    '23:59:59.999'::TIME(9) AS clock_time,
    '1969-12-31 23:59:59.999'::TIMESTAMP_NTZ(9) AS clock_timestamp,
    '2026-03-05 23:30:00'::TIMESTAMP_NTZ(9) AS zoned_instant_utc,
    TRUE AS today_is_after_2020,
    0 AS today_minus_today,
    TRUE AS system_clock_is_after_2020
  UNION ALL
  SELECT
    'CT-03' AS text_id,
    '2026-12-31'::DATE AS clock_date,
    '00:00:00'::TIME(9) AS clock_time,
    '2026-12-31 00:00:00'::TIMESTAMP_NTZ(9) AS clock_timestamp,
    '2027-01-01 05:00:00'::TIMESTAMP_NTZ(9) AS zoned_instant_utc,
    TRUE AS today_is_after_2020,
    0 AS today_minus_today,
    TRUE AS system_clock_is_after_2020
  UNION ALL
  SELECT
    'CT-04' AS text_id,
    CAST(NULL AS DATE) AS clock_date,
    CAST(NULL AS TIME(9)) AS clock_time,
    CAST(NULL AS TIMESTAMP_NTZ(9)) AS clock_timestamp,
    CAST(NULL AS TIMESTAMP_NTZ(9)) AS zoned_instant_utc,
    TRUE AS today_is_after_2020,
    0 AS today_minus_today,
    TRUE AS system_clock_is_after_2020
)
