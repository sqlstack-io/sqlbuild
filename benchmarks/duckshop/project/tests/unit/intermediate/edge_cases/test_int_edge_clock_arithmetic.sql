TEST (name "int_edge_clock_arithmetic__adds_diffs_and_formats_fine_clock_values");

WITH __ref__stg_edge_date_texts AS (
  SELECT
    v.text_id,
    NULL AS date_text,
    NULL AS zoned_text,
    v.wall_clock::TIMESTAMP_NTZ(3) AS wall_clock,
    v.wall_clock_text,
    v.local_time::TIME(3) AS local_time,
    v.fine_time::TIME(9) AS fine_time
  FROM VALUES
    ('CK-01', '2026-03-05 23:30:00.123', '2026-03-05 23:30:00.1239', '23:59:59.999', '23:59:59.9999999'),
    ('CK-02', '2026-03-05 23:30:00.000', '2026-03-05 23:30:00.9', '00:00:00', '12:00:00.0000005'),
    ('CK-03', '2026-03-05 23:30:00.120', '2026-03-05 23:30:00.12', '12:30:15.250', '00:00:00.000000001'),
    ('CK-04', '1969-12-31 23:59:59.999', '1969-12-31 23:59:59.999', '23:30:00', '23:59:59.999999999'),
    ('CK-05', '2026-12-31 23:59:59.999', '2026-12-31 23:59:59.999000', '06:00:00.001', '06:00:00.0000001'),
    ('CK-06', NULL, NULL, NULL, NULL) AS v(text_id, wall_clock, wall_clock_text, local_time, fine_time)
),

__expected__int_edge_clock_arithmetic AS (
  SELECT
    NULL AS text_id,
    NULL AS time_plus_minutes,
    NULL AS time_minus_hours,
    NULL AS time_plus_millisecond,
    NULL AS time_add_type,
    NULL AS time_cast_type,
    NULL AS wall_clock_plus_day,
    NULL AS wall_clock_add_type,
    NULL AS years_to_2027,
    NULL AS months_to_2027,
    NULL AS weeks_to_2027,
    NULL AS hours_to_2027,
    NULL AS milliseconds_to_2027,
    NULL AS minutes_to_midnight,
    NULL AS fine_time_micro_text,
    NULL AS fine_time_second_text,
    NULL AS fine_time_milli_text,
    NULL AS text_clock_ff3,
    NULL AS wall_clock_bare_ff,
    NULL AS wall_clock_equals_text,
    NULL AS whole_second_equals_text,
    NULL AS whole_second_before_text,
    NULL AS epoch_millis_type,
    NULL AS bare_timestamp_type
)
