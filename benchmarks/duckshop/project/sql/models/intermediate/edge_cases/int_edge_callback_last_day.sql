MODEL (
  description """
    One row per gateway callback whose zoned time falls within the 24 hours before the current
    time
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    is_recent (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    (g.event_at >= DATEADD(hour, -24, CURRENT_TIMESTAMP()))::BOOLEAN AS is_recent
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
