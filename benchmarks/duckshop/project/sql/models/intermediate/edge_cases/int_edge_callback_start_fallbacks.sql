MODEL (
  description """
    One row per gateway callback with its nanosecond time, or its free text when that is
    missing, as a start time
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    start_at (type TIMESTAMP_NTZ(9)),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    COALESCE(g.event_at_ns, g.event_time_text)::TIMESTAMP_NTZ(9) AS start_at
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
