MODEL (
  description """
    One row per gateway callback with the start of the day of its zoned time
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    day_start (type TIMESTAMP_TZ),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    DATE_TRUNC('day', g.event_at) AS day_start
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
