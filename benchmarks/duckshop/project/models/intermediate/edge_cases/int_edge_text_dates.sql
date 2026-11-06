MODEL (
  description """
    One row per partner date text read with TRY_TO_DATE under numeric and month-name formats:
    unpadded days and months, two-digit years, ISO T and Z separators, trailing letters, a long s in
    a month name, mixed separators and runs of spaces
  """,
  tags [edge_case],
  columns (
    text_id (type VARCHAR, nullable false, audits [not_null, unique]),
    date_text (type VARCHAR),
    day_month_dots (type DATE),
    iso_date (type DATE),
    day_month_short_year (type DATE),
    month_day_slashes (type DATE),
    day_month_name (type DATE),
    month_name_first (type DATE),
    auto_date (type DATE),
  ),
);

WITH texts AS (
  SELECT
    t.text_id,
    t.date_text
  FROM __ref("stg_edge_date_texts") AS t
),

final AS (
  SELECT
    t.text_id,
    t.date_text,
    TRY_TO_DATE(t.date_text, 'DD.MM.YYYY') AS day_month_dots,
    TRY_TO_DATE(t.date_text, 'YYYY-MM-DD') AS iso_date,
    TRY_TO_DATE(t.date_text, 'DD/MM/YY') AS day_month_short_year,
    TRY_TO_DATE(t.date_text, 'MM/DD/YYYY') AS month_day_slashes,
    TRY_TO_DATE(t.date_text, 'DD MON YYYY') AS day_month_name,
    TRY_TO_DATE(t.date_text, 'MMMM DD YYYY') AS month_name_first,
    TRY_TO_DATE(t.date_text) AS auto_date
  FROM texts AS t
)

SELECT *
FROM final
