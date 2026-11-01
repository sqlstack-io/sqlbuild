TEST (name "int_edge_zoned_texts__reads_offsets_back_as_utc_instants_dates_and_years");

WITH __ref__stg_edge_date_texts AS (
  SELECT
    v.text_id,
    NULL AS date_text,
    v.zoned_text,
    v.wall_clock::TIMESTAMP_NTZ(3) AS wall_clock,
    v.wall_clock_text,
    NULL::TIME(3) AS local_time,
    NULL::TIME(9) AS fine_time
  FROM VALUES
    ('ZT-01', '2026-03-06T08:30:00+09:00', '2026-03-05 23:30:00.000', '2026-03-05 23:30:00'),
    ('ZT-02', '2026-03-05T23:30:00Z', '2026-03-05 23:00:00.000', '2026-03-05 23:30:00'),
    ('ZT-03', '2026-03-05 23:30:00+00:00', '2026-03-06 00:00:00.000', '2026-03-05 23:30:00'),
    ('ZT-04', '2026-03-05 23:30:00 +0000', '2026-03-05 23:30:00.000', '2026-03-05 23:30:00.000001'),
    ('ZT-05', '2026-03-05T18:00:00-05:30', '2026-03-05 23:29:59.999', '2026-03-05 23:30:00'),
    ('ZT-06', '2026-12-31T23:30:00-05:30', '2027-01-01 00:00:00.000', '2027-01-01 05:00:00'),
    ('ZT-07', '2026-03-05T23:30:00', '2026-03-05 23:30:00.000', '2026-03-05 23:30:00'),
    ('ZT-08', NULL, '1969-12-31 23:59:59.999', '1969-12-31 23:59:59.999'),
    ('ZT-09', 'not a time', NULL, NULL) AS v(text_id, zoned_text, wall_clock, wall_clock_text)
),

__expected__int_edge_zoned_texts AS (
  SELECT
    NULL AS text_id,
    NULL AS zoned,
    NULL AS local_zoned,
    NULL AS zoned_utc,
    NULL AS local_zoned_utc,
    NULL AS zoned_date,
    NULL AS local_zoned_date,
    NULL AS zoned_year,
    NULL AS local_zoned_year,
    NULL AS zoned_day_start,
    NULL AS local_zoned_day_start,
    NULL AS zoned_equals_wall_text,
    NULL AS local_zoned_equals_wall_text,
    NULL AS zoned_after_wall_clock,
    NULL AS local_zoned_after_wall_clock,
    NULL AS zoned_from_next_day,
    NULL AS local_zoned_from_next_day,
    NULL AS first_zoned_date,
    NULL AS zoned_lower_utc,
    NULL AS wall_clock_utc_hour
)
