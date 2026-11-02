MODEL (
  description """
    One row per fixed period, the first of last month beside text in one VALUES column
  """,
  tags [edge_case],
  columns (
    period_id (type VARCHAR, nullable false, audits [not_null, unique]),
    period_start (type VARCHAR),
  ),
);

WITH periods AS (
  SELECT v.period_id, v.period_start
  FROM VALUES
    ('P-1', DATEADD('month', -1, DATE_TRUNC('month', '2026-03-15'::DATE))),
    ('P-2', '2026-01-01') AS v(period_id, period_start)
),

final AS (
  SELECT
    p.period_id,
    p.period_start::VARCHAR AS period_start
  FROM periods AS p
)

SELECT *
FROM final
