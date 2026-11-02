MODEL (
  description """
    One row per zoned partner timestamp text read as TIMESTAMP_TZ and TIMESTAMP_LTZ, with T, Z,
    +00:00, +0000, +09:00 and -05:30 spellings, read back as the instant at UTC, its date, year and
    day start, and compared with wall-clock text, a TIMESTAMP_NTZ and a DATE. Also the date of the
    first zoned value by window and the UTC hour of a wall-clock time just before 1970.
    CONVERT_TIMEZONE with a lower-case 'utc' is left out: Snowflake rejects it as an unknown time
    zone. TIMESTAMP_LTZ results depend on the session time zone, which the edge target pins to UTC
  """,
  tags [edge_case, known_divergence],
  columns (
    text_id (type VARCHAR, nullable false, audits [not_null, unique]),
    zoned_text (type VARCHAR),
    zoned (type TIMESTAMP_TZ),
    local_zoned (type TIMESTAMP_LTZ),
    zoned_utc (type TIMESTAMP_NTZ),
    local_zoned_utc (type TIMESTAMP_NTZ),
    zoned_date (type DATE),
    local_zoned_date (type DATE),
    zoned_year (type NUMBER(38, 0)),
    local_zoned_year (type NUMBER(38, 0)),
    zoned_day_start (type TIMESTAMP_TZ),
    local_zoned_day_start (type TIMESTAMP_LTZ),
    zoned_equals_wall_text (type BOOLEAN),
    local_zoned_equals_wall_text (type BOOLEAN),
    zoned_after_wall_clock (type BOOLEAN),
    local_zoned_after_wall_clock (type BOOLEAN),
    zoned_from_next_day (type BOOLEAN),
    local_zoned_from_next_day (type BOOLEAN),
    first_zoned_date (type DATE),
    wall_clock_utc_hour (type NUMBER(38, 0)),
  ),
);

WITH texts AS (
  SELECT
    t.text_id,
    t.zoned_text,
    t.wall_clock,
    t.wall_clock_text
  FROM __ref("stg_edge_date_texts") AS t
),

parsed AS (
  SELECT
    t.text_id,
    t.zoned_text,
    t.wall_clock,
    t.wall_clock_text,
    TRY_TO_TIMESTAMP_TZ(t.zoned_text) AS zoned,
    TRY_TO_TIMESTAMP_LTZ(t.zoned_text) AS local_zoned
  FROM texts AS t
),

final AS (
  SELECT
    p.text_id,
    p.zoned_text,
    p.zoned,
    p.local_zoned,
    CONVERT_TIMEZONE('UTC', p.zoned)::TIMESTAMP_NTZ AS zoned_utc,
    CONVERT_TIMEZONE('UTC', p.local_zoned)::TIMESTAMP_NTZ AS local_zoned_utc,
    TO_DATE(p.zoned) AS zoned_date,
    TO_DATE(p.local_zoned) AS local_zoned_date,
    YEAR(p.zoned)::NUMBER(38, 0) AS zoned_year,
    YEAR(p.local_zoned)::NUMBER(38, 0) AS local_zoned_year,
    DATE_TRUNC('day', p.zoned) AS zoned_day_start,
    DATE_TRUNC('day', p.local_zoned) AS local_zoned_day_start,
    p.zoned = p.wall_clock_text::TIMESTAMP_TZ AS zoned_equals_wall_text,
    p.local_zoned = p.wall_clock_text::TIMESTAMP_LTZ AS local_zoned_equals_wall_text,
    p.zoned > p.wall_clock AS zoned_after_wall_clock,
    p.local_zoned > p.wall_clock AS local_zoned_after_wall_clock,
    p.zoned >= '2026-03-06'::DATE AS zoned_from_next_day,
    p.local_zoned >= '2026-03-06'::DATE AS local_zoned_from_next_day,
    FIRST_VALUE(p.zoned) OVER (
      ORDER BY p.text_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
    )::DATE AS first_zoned_date,
    DATE_PART(hour, CONVERT_TIMEZONE('UTC', p.wall_clock))::NUMBER(38, 0) AS wall_clock_utc_hour
  FROM parsed AS p
)

SELECT *
FROM final
