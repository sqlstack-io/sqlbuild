MODEL (
  description """
    One row per partner clock reading with time arithmetic at its edges: DATEADD and TIMEADD on a
    TIME(3) and the type they return, DATEDIFF across units, TIME(9) values formatted with fewer
    fraction digits (Snowflake truncates), FF3 and bare FF on timestamps of precision 3 and 9, a
    TIMESTAMP_NTZ(3) and its NTZ(0) cast compared with text holding finer fractions (cast to
    TIMESTAMP_NTZ, which Snowflake reads at precision 9), and the types of casts to TIME(3),
    TO_TIMESTAMP of a NUMBER(13,3) and a bare TIMESTAMP cast
  """,
  tags [edge_case],
  columns (
    text_id (type VARCHAR, nullable false, audits [not_null, unique]),
    time_plus_minutes (type TIME),
    time_minus_hours (type TIME),
    time_plus_millisecond (type TIME),
    time_add_type (type VARCHAR),
    time_cast_type (type VARCHAR),
    wall_clock_plus_day (type TIMESTAMP_NTZ),
    wall_clock_add_type (type VARCHAR),
    years_to_2027 (type NUMBER(38, 0)),
    months_to_2027 (type NUMBER(38, 0)),
    weeks_to_2027 (type NUMBER(38, 0)),
    hours_to_2027 (type NUMBER(38, 0)),
    milliseconds_to_2027 (type NUMBER(38, 0)),
    minutes_to_midnight (type NUMBER(38, 0)),
    fine_time_micro_text (type VARCHAR),
    fine_time_second_text (type VARCHAR),
    fine_time_milli_text (type VARCHAR),
    text_clock_ff3 (type VARCHAR),
    wall_clock_bare_ff (type VARCHAR),
    wall_clock_equals_text (type BOOLEAN),
    whole_second_equals_text (type BOOLEAN),
    whole_second_before_text (type BOOLEAN),
    epoch_millis_type (type VARCHAR),
    bare_timestamp_type (type VARCHAR),
  ),
);

WITH clocks AS (
  SELECT
    c.text_id,
    c.wall_clock,
    c.wall_clock_text,
    c.local_time,
    c.fine_time
  FROM __ref("stg_edge_date_texts") AS c
),

final AS (
  SELECT
    c.text_id,
    DATEADD('minute', 30, c.local_time)::TIME AS time_plus_minutes,
    TIMEADD('hour', -10, c.local_time)::TIME AS time_minus_hours,
    DATEADD('millisecond', 1, c.local_time)::TIME AS time_plus_millisecond,
    REGEXP_REPLACE(
      SYSTEM$TYPEOF(DATEADD('minute', 30, c.local_time)),
      '\\[.*\\]$',
      ''
    )::VARCHAR AS time_add_type,
    REGEXP_REPLACE(
      SYSTEM$TYPEOF(c.wall_clock::TIME(3)),
      '\\[.*\\]$',
      ''
    )::VARCHAR AS time_cast_type,
    DATEADD('day', 1, c.wall_clock) AS wall_clock_plus_day,
    REGEXP_REPLACE(
      SYSTEM$TYPEOF(DATEADD('day', 1, c.wall_clock)),
      '\\[.*\\]$',
      ''
    )::VARCHAR AS wall_clock_add_type,
    DATEDIFF(
      'year',
      c.wall_clock,
      '2027-01-01 00:00:00'::TIMESTAMP_NTZ
    )::NUMBER(38, 0) AS years_to_2027,
    DATEDIFF(
      'month',
      c.wall_clock,
      '2027-01-01 00:00:00'::TIMESTAMP_NTZ
    )::NUMBER(38, 0) AS months_to_2027,
    DATEDIFF(
      'week',
      c.wall_clock,
      '2027-01-01 00:00:00'::TIMESTAMP_NTZ
    )::NUMBER(38, 0) AS weeks_to_2027,
    DATEDIFF(
      'hour',
      c.wall_clock,
      '2027-01-01 00:00:00'::TIMESTAMP_NTZ
    )::NUMBER(38, 0) AS hours_to_2027,
    DATEDIFF(
      'millisecond',
      c.wall_clock,
      '2027-01-01 00:00:00'::TIMESTAMP_NTZ
    )::NUMBER(38, 0) AS milliseconds_to_2027,
    DATEDIFF(
      'minute',
      c.local_time,
      '23:59:59'::TIME
    )::NUMBER(38, 0) AS minutes_to_midnight,
    TO_CHAR(c.fine_time, 'HH24:MI:SS.FF6')::VARCHAR AS fine_time_micro_text,
    TO_CHAR(c.fine_time, 'HH24:MI:SS')::VARCHAR AS fine_time_second_text,
    TO_CHAR(c.fine_time, 'HH24:MI:SS.FF3')::VARCHAR AS fine_time_milli_text,
    TO_CHAR(
      TRY_TO_TIMESTAMP_NTZ(c.wall_clock_text),
      'YYYY-MM-DD HH24:MI:SS.FF3'
    )::VARCHAR AS text_clock_ff3,
    TO_CHAR(c.wall_clock, 'HH24:MI:SS.FF')::VARCHAR AS wall_clock_bare_ff,
    c.wall_clock = c.wall_clock_text::TIMESTAMP_NTZ AS wall_clock_equals_text,
    c.wall_clock::TIMESTAMP_NTZ(0) = c.wall_clock_text::TIMESTAMP_NTZ AS whole_second_equals_text,
    c.wall_clock::TIMESTAMP_NTZ(0) < c.wall_clock_text::TIMESTAMP_NTZ AS whole_second_before_text,
    REGEXP_REPLACE(
      SYSTEM$TYPEOF(TO_TIMESTAMP(1772359200.123::NUMBER(13, 3))),
      '\\[.*\\]$',
      ''
    )::VARCHAR AS epoch_millis_type,
    REGEXP_REPLACE(
      SYSTEM$TYPEOF('2026-03-01 10:15:00'::TIMESTAMP),
      '\\[.*\\]$',
      ''
    )::VARCHAR AS bare_timestamp_type
  FROM clocks AS c
)

SELECT *
FROM final
