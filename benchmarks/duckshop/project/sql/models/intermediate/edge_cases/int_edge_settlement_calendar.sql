MODEL (
  description """
    One row per payment gateway callback with its business date placed in the settlement calendar:
    month ends built from out-of-range date parts, renewal dates a month on, and the day, ISO and
    week-of-year numbering around a year boundary
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    business_date (type DATE),
    month_end (type DATE),
    previous_month_end (type DATE),
    next_year_start (type DATE),
    day_before_month (type DATE),
    end_of_day (type TIMESTAMP_NTZ),
    renews_on (type DATE),
    renews_on_add_months (type DATE),
    last_day_of_month (type DATE),
    day_of_week (type INT),
    iso_day_of_week (type INT),
    week_of_year (type INT),
    iso_week (type INT),
    year_of_week (type INT),
    iso_year_of_week (type INT),
    week_start (type DATE),
  ),
);

WITH callbacks AS (
  SELECT
    s.callback_id,
    s.business_date
  FROM __ref("stg_edge_gateway_callbacks") AS s
),

final AS (
  SELECT
    c.callback_id,
    c.business_date,
    DATE_FROM_PARTS(YEAR(c.business_date), MONTH(c.business_date) + 1, 0) AS month_end,
    DATE_FROM_PARTS(YEAR(c.business_date), MONTH(c.business_date), 0) AS previous_month_end,
    DATE_FROM_PARTS(YEAR(c.business_date), 13, 1) AS next_year_start,
    DATE_FROM_PARTS(YEAR(c.business_date), MONTH(c.business_date), -1) AS day_before_month,
    TIMESTAMP_NTZ_FROM_PARTS(
      YEAR(c.business_date),
      MONTH(c.business_date),
      DAY(c.business_date),
      24,
      0,
      0
    ) AS end_of_day,
    DATEADD('month', 1, c.business_date)::DATE AS renews_on,
    ADD_MONTHS(c.business_date, 1)::DATE AS renews_on_add_months,
    LAST_DAY(c.business_date)::DATE AS last_day_of_month,
    DAYOFWEEK(c.business_date)::INT AS day_of_week,
    DAYOFWEEKISO(c.business_date)::INT AS iso_day_of_week,
    WEEKOFYEAR(c.business_date)::INT AS week_of_year,
    WEEKISO(c.business_date)::INT AS iso_week,
    YEAROFWEEK(c.business_date)::INT AS year_of_week,
    YEAROFWEEKISO(c.business_date)::INT AS iso_year_of_week,
    DATE_TRUNC('week', c.business_date)::DATE AS week_start
  FROM callbacks AS c
)

SELECT *
FROM final
