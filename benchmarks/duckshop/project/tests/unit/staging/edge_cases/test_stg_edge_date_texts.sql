TEST (name "stg_edge_date_texts__keeps_clock_precisions");

WITH __source__edge__date_texts AS (
  SELECT
    v.text_id,
    v.date_text,
    v.zoned_text,
    v.wall_clock::TIMESTAMP_NTZ AS wall_clock,
    v.wall_clock_text,
    v.local_time::TIME AS local_time,
    v.fine_time::TIME AS fine_time,
    '2026-03-06 00:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('DT-01', '5.3.2026', '2026-03-05T23:30:00Z', '2026-03-05 23:30:00.123', '2026-03-05 23:30:00.1239', '23:59:59.999', '23:59:59.9999999'),
    ('DT-02', NULL, NULL, NULL, NULL, NULL, NULL) AS v(text_id, date_text, zoned_text, wall_clock, wall_clock_text, local_time, fine_time)
),

__expected__stg_edge_date_texts AS (
  SELECT
    'DT-01' AS text_id,
    '5.3.2026' AS date_text,
    '2026-03-05T23:30:00Z' AS zoned_text,
    '2026-03-05 23:30:00.123'::TIMESTAMP_NTZ(3) AS wall_clock,
    '2026-03-05 23:30:00.1239' AS wall_clock_text,
    '23:59:59.999'::TIME(3) AS local_time,
    '23:59:59.9999999'::TIME(9) AS fine_time
  UNION ALL
  SELECT
    'DT-02' AS text_id,
    CAST(NULL AS VARCHAR) AS date_text,
    CAST(NULL AS VARCHAR) AS zoned_text,
    CAST(NULL AS TIMESTAMP_NTZ(3)) AS wall_clock,
    CAST(NULL AS VARCHAR) AS wall_clock_text,
    CAST(NULL AS TIME(3)) AS local_time,
    CAST(NULL AS TIME(9)) AS fine_time
)
