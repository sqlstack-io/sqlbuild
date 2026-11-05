MODEL (
  description """
    One row per gateway callback with its free text read as a date when it can be, and its zoned
    time in an object
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    text_date (type DATE),
    event_json (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    TRY_CAST(g.event_time_text AS DATE) AS text_date,
    TO_JSON(OBJECT_CONSTRUCT('at', g.event_at)) AS event_json
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
