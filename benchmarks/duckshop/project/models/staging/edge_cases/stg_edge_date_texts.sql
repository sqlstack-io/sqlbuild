MODEL (
  description """
    One row per partner date and time text, with the clock columns kept at their declared precisions
    so later models can read them as text, zoned times and times of day
  """,
  tags [edge_case],
  columns (
    text_id (type VARCHAR, nullable false, audits [not_null, unique]),
    date_text (type VARCHAR),
    zoned_text (type VARCHAR),
    wall_clock (type TIMESTAMP_NTZ(3)),
    wall_clock_text (type VARCHAR),
    local_time (type TIME(3)),
    fine_time (type TIME(9)),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__date_texts")
),

final AS (
  SELECT
    s.text_id,
    s.date_text,
    s.zoned_text,
    s.wall_clock,
    s.wall_clock_text,
    s.local_time,
    s.fine_time
  FROM src AS s
)

SELECT *
FROM final
