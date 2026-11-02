MODEL (
  description """
    One row per gateway callback with whether the current time, converted from UTC to London
    wall time, is known: a fact that holds on any day
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    london_now_is_known (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    (CONVERT_TIMEZONE('UTC', 'Europe/London', CURRENT_TIMESTAMP())::TIMESTAMP_NTZ IS NOT NULL)::BOOLEAN
      AS london_now_is_known
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
