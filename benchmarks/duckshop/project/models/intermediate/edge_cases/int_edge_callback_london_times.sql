MODEL (
  description """
    One row per gateway callback with its zoned time converted from UTC to London wall time
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    london_time (type TIMESTAMP_NTZ),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    CONVERT_TIMEZONE('UTC', 'Europe/London', g.event_at)::TIMESTAMP_NTZ AS london_time
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
