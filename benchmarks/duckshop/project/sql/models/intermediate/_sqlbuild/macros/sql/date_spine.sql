MACRO (
  description "One row per calendar day from a start date for a fixed number of days",
  arguments (start_date VARCHAR CONST, day_count INT CONST),
);

WITH spine_offsets AS (
  SELECT ARRAY_GENERATE_RANGE(0, @day_count) AS day_offsets
),

date_spine AS (
  SELECT
    DATEADD(
      'day',
      d.value::INT,
      @start_date::DATE
    )::DATE AS calendar_date
  FROM spine_offsets AS o, LATERAL FLATTEN(input => o.day_offsets) AS d
)
