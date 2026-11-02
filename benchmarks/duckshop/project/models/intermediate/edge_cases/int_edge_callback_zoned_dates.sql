MODEL (
  description """
    One row per gateway callback whose zoned time falls on or after 1 March 2026 as a date
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    event_date (type DATE),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    TO_DATE(g.event_at) AS event_date
  FROM __ref("stg_edge_gateway_callbacks") AS g
  WHERE TO_DATE(g.event_at) >= '2026-03-01'::DATE
)

SELECT *
FROM final
