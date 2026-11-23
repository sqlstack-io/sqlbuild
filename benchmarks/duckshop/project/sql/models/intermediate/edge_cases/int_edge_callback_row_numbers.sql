MODEL (
  description """
    One row per gateway callback with its wall time as text and a SEQ8 number unique in the
    result
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    wall_time_text (type VARCHAR),
    has_row_number (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    CAST(g.local_wall_time AS VARCHAR) AS wall_time_text,
    (SEQ8() IS NOT NULL)::BOOLEAN AS has_row_number
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
