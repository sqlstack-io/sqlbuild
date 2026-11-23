MODEL (
  description """
    One row per merchant zone with its callbacks' wall times in callback order, a NULL for each
    callback without one
  """,
  tags [edge_case],
  columns (
    merchant_zone (type VARCHAR, nullable false, audits [not_null, unique]),
    wall_times (type ARRAY),
  ),
);

WITH final AS (
  SELECT
    g.merchant_zone,
    ARRAY_AGG(IFF(g.callback_id IS NOT NULL, g.local_wall_time, NULL))
      WITHIN GROUP (ORDER BY g.callback_id)::ARRAY AS wall_times
  FROM __ref("stg_edge_gateway_callbacks") AS g
  GROUP BY
    g.merchant_zone
)

SELECT *
FROM final
