MODEL (
  description """
    One row per calendar day of the three calendar years from the start of trading history, with
    ISO week and fiscal calendar attributes and a holiday flag per region.
  """,
  contract enforced,
  constants (
    _calendar_days 1095,
    _fiscal_year_start_month 4,
  ),
  columns (
    calendar_date (type DATE, nullable false, audits [not_null, unique]),
    day_of_week_iso (type INT),
    day_name (type VARCHAR),
    is_weekend (type BOOLEAN),
    iso_year (type INT),
    iso_week (type INT),
    month_start (type DATE),
    month_end (type DATE),
    quarter_label (type VARCHAR),
    fiscal_year (type INT),
    fiscal_quarter (type INT),
    holiday_regions (type ARRAY),
    is_holiday_somewhere (type BOOLEAN),
    is_on_or_before_valuation (type BOOLEAN),
  ),
);

WITH holidays AS (
  SELECT *
  FROM __seed("holiday_calendar")
),

generated AS (
  SELECT
    DATEADD(
      'day',
      ROW_NUMBER() OVER (ORDER BY SEQ4()) - 1,
      @const("history_start_date")::DATE
    ) AS calendar_date
  FROM TABLE(GENERATOR(ROWCOUNT => @const("_calendar_days")))
),

holiday_days AS (
  SELECT
    h.holiday_date,
    ARRAY_AGG(DISTINCT h.region_code) WITHIN GROUP (ORDER BY h.region_code) AS holiday_regions
  FROM holidays AS h
  GROUP BY
    h.holiday_date
),

attributes AS (
  SELECT
    g.calendar_date,
    DAYOFWEEKISO(g.calendar_date) AS day_of_week_iso,
    DAYNAME(g.calendar_date) AS day_name,
    YEAROFWEEKISO(g.calendar_date) AS iso_year,
    WEEKISO(g.calendar_date) AS iso_week,
    DATE_TRUNC('month', g.calendar_date) AS month_start,
    LAST_DAY(g.calendar_date, 'month') AS month_end,
    YEAR(g.calendar_date) || '-Q' || QUARTER(g.calendar_date) AS quarter_label,
    YEAR(
      DATEADD('month', 12 - @const("_fiscal_year_start_month") + 1, g.calendar_date)
    ) AS fiscal_year,
    CEIL(
      MONTH(DATEADD('month', 12 - @const("_fiscal_year_start_month") + 1, g.calendar_date)) / 3
    ) AS fiscal_quarter
  FROM generated AS g
),

final AS (
  SELECT
    a.calendar_date::DATE AS calendar_date,
    a.day_of_week_iso::INT AS day_of_week_iso,
    a.day_name::VARCHAR AS day_name,
    (
      a.day_of_week_iso >= 6
    )::BOOLEAN AS is_weekend,
    a.iso_year::INT AS iso_year,
    a.iso_week::INT AS iso_week,
    a.month_start::DATE AS month_start,
    a.month_end::DATE AS month_end,
    a.quarter_label::VARCHAR AS quarter_label,
    a.fiscal_year::INT AS fiscal_year,
    a.fiscal_quarter::INT AS fiscal_quarter,
    COALESCE(h.holiday_regions, ARRAY_CONSTRUCT())::ARRAY AS holiday_regions,
    (
      h.holiday_date IS NOT NULL
    )::BOOLEAN AS is_holiday_somewhere,
    (
      a.calendar_date <= '@@valuation_date'::DATE
    )::BOOLEAN AS is_on_or_before_valuation
  FROM attributes AS a
  LEFT JOIN holiday_days AS h
    ON a.calendar_date = h.holiday_date
)

SELECT *
FROM final
