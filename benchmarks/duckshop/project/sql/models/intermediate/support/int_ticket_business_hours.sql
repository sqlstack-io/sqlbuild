MODEL (
  description """
    One row per ticket with first-response time counted in support business hours (08:00 to 20:00
    Berlin time on working days, regional holidays excluded), alongside the calendar time.
  """,
  constants (
    _support_timezone "Europe/Berlin",
    _day_start_hour 8,
    _day_end_hour 20,
  ),
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    created_at_support_local (type TIMESTAMP_NTZ),
    first_response_at_support_local (type TIMESTAMP_NTZ),
    calendar_minutes_to_response (type INT),
    business_minutes_to_response (type INT),
    created_outside_hours (type BOOLEAN),
  ),
);

WITH first_responses AS (
  SELECT *
  FROM __ref("int_ticket_first_response")
),

holidays AS (
  SELECT *
  FROM __seed("holiday_calendar")
),

@sql.date_spine(@const("history_start_date"), 730)

local_times AS (
  SELECT
    f.ticket_id,
    CONVERT_TIMEZONE(
      'UTC',
      @const("_support_timezone"),
      f.created_at_utc
    )::TIMESTAMP_NTZ AS created_local,
    CONVERT_TIMEZONE(
      'UTC',
      @const("_support_timezone"),
      f.first_response_at_utc
    )::TIMESTAMP_NTZ AS response_local
  FROM first_responses AS f
), local_dates AS (
  SELECT
    l.ticket_id,
    l.created_local,
    l.response_local,
    l.created_local::DATE AS created_date,
    l.response_local::DATE AS response_date
  FROM local_times AS l
  WHERE l.response_local IS NOT NULL
), eu_holidays AS (
  SELECT h.holiday_date
  FROM holidays AS h
  WHERE h.region_code = 'EU'
), working_days AS (
  SELECT d.calendar_date
  FROM date_spine AS d
  LEFT JOIN eu_holidays AS h
    ON d.calendar_date = h.holiday_date
  WHERE
    DAYOFWEEKISO(d.calendar_date) <= 5
    AND h.holiday_date IS NULL
), day_windows AS (
  SELECT
    ld.ticket_id,
    GREATEST(
      ld.created_local,
      TIMESTAMP_NTZ_FROM_PARTS(
        YEAR(w.calendar_date),
        MONTH(w.calendar_date),
        DAY(w.calendar_date),
        @const("_day_start_hour"),
        0,
        0
      )
    ) AS window_start,
    LEAST(
      ld.response_local,
      TIMESTAMP_NTZ_FROM_PARTS(
        YEAR(w.calendar_date),
        MONTH(w.calendar_date),
        DAY(w.calendar_date),
        @const("_day_end_hour"),
        0,
        0
      )
    ) AS window_end
  FROM local_dates AS ld
  INNER JOIN working_days AS w
    ON w.calendar_date >= ld.created_date
    AND w.calendar_date <= ld.response_date
), business_minutes AS (
  SELECT
    dw.ticket_id,
    SUM(
      GREATEST(DATEDIFF('minute', dw.window_start, dw.window_end), 0)
    ) AS business_minutes_to_response
  FROM day_windows AS dw
  GROUP BY
    dw.ticket_id
), final AS (
  SELECT
    l.ticket_id,
    l.created_local AS created_at_support_local,
    l.response_local AS first_response_at_support_local,
    DATEDIFF('minute', l.created_local, l.response_local)::INT AS calendar_minutes_to_response,
    COALESCE(b.business_minutes_to_response, 0)::INT AS business_minutes_to_response,
    (
      HOUR(l.created_local) NOT BETWEEN @const("_day_start_hour") AND @const("_day_end_hour") - 1
      OR DAYOFWEEKISO(l.created_local) > 5
    ) AS created_outside_hours
  FROM local_times AS l
  LEFT JOIN business_minutes AS b
    ON l.ticket_id = b.ticket_id
)
SELECT *
FROM final
