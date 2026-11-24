MODEL (
  description """
    One row per payment gateway callback with its settlement period measured by calendar boundaries
    crossed, not elapsed time, in days, months and minutes
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    days_crossed (type INT),
    months_crossed (type INT),
    hours_crossed (type INT),
    minutes_elapsed (type INT),
    days_backwards (type INT),
    period_end_month (type DATE),
  ),
);

WITH callbacks AS (
  SELECT
    s.callback_id,
    s.period_start,
    s.period_end
  FROM __ref("stg_edge_gateway_callbacks") AS s
),

final AS (
  SELECT
    c.callback_id,
    DATEDIFF('day', c.period_start, c.period_end)::INT AS days_crossed,
    DATEDIFF('month', c.period_start, c.period_end)::INT AS months_crossed,
    DATEDIFF('hour', c.period_start, c.period_end)::INT AS hours_crossed,
    TIMESTAMPDIFF('minute', c.period_start, c.period_end)::INT AS minutes_elapsed,
    DATEDIFF('day', c.period_end, c.period_start)::INT AS days_backwards,
    DATE_TRUNC('month', c.period_end)::DATE AS period_end_month
  FROM callbacks AS c
)

SELECT *
FROM final
