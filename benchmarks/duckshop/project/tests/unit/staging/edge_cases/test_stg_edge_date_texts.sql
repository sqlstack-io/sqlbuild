TEST (name "stg_edge_date_texts__keeps_clock_precisions");

WITH __source__edge__date_texts AS (
  SELECT
    v.text_id,
    v.date_text,
    v.zoned_text,
    v.wall_clock::TIMESTAMP_NTZ(3) AS wall_clock,
    v.wall_clock_text,
    v.local_time::TIME(3) AS local_time,
    v.fine_time::TIME(9) AS fine_time,
    '2026-03-06 00:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('DT-01', '5.3.2026', '2026-03-05T23:30:00Z', '2026-03-05 23:30:00.123', '2026-03-05 23:30:00.1239', '23:59:59.999', '23:59:59.9999999'),
    ('DT-02', NULL, NULL, NULL, NULL, NULL, NULL) AS v(text_id, date_text, zoned_text, wall_clock, wall_clock_text, local_time, fine_time)
),

__expected__stg_edge_date_texts AS (
  SELECT
    NULL AS text_id,
    NULL AS date_text,
    NULL AS zoned_text,
    NULL AS wall_clock,
    NULL AS wall_clock_text,
    NULL AS local_time,
    NULL AS fine_time
)
