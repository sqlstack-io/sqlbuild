TEST (name "int_edge_text_dates__reads_numeric_and_month_name_formats");

WITH __ref__stg_edge_date_texts AS (
  SELECT
    v.text_id,
    v.date_text,
    NULL AS zoned_text,
    NULL::TIMESTAMP_NTZ(3) AS wall_clock,
    NULL AS wall_clock_text,
    NULL::TIME(3) AS local_time,
    NULL::TIME(9) AS fine_time
  FROM VALUES
    ('DT-01', '5.3.2026'),
    ('DT-02', '05.03.26'),
    ('DT-03', '2026-3-5'),
    ('DT-04', '05/03/26'),
    ('DT-05', '5/3/26'),
    ('DT-06', '03/05/2026'),
    ('DT-07', '2026-10-05T10:00'),
    ('DT-08', '2025-10-05Z'),
    ('DT-09', '2025-10-05t'),
    ('DT-10', '2025-10-05x'),
    ('DT-11', '2025-10-05T10:00x'),
    ('DT-12', '05 Augu\u017ft 2024'),
    ('DT-13', '05 August 2024'),
    ('DT-14', '05-Mar/2024'),
    ('DT-15', '05Mar 2024'),
    ('DT-16', '05 Mar2024'),
    ('DT-17', '   05  Mar  2024   '),
    ('DT-18', 'March   05   2024'),
    ('DT-19', '5 Mar 20245'),
    ('DT-20', 'March 005 2024'),
    ('DT-21', '15 oktober 2025'),
    ('DT-22', '2026-02-30'),
    ('DT-23', NULL) AS v(text_id, date_text)
),

__expected__int_edge_text_dates AS (
  SELECT
    NULL AS text_id,
    NULL AS day_month_dots,
    NULL AS iso_date,
    NULL AS day_month_short_year,
    NULL AS month_day_slashes,
    NULL AS day_month_name,
    NULL AS month_name_first,
    NULL AS auto_date
)
