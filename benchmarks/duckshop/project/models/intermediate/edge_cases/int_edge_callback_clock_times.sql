MODEL (
  description """
    One row per gateway callback with its free text read as a date, and the time of day of the
    text read as a zoned timestamp
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    text_date (type DATE),
    text_time (type TIME),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    TO_DATE(CAST(g.business_date AS VARCHAR)) AS text_date,
    TO_TIME(TRY_TO_TIMESTAMP_TZ(g.event_time_text))::TIME AS text_time
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
