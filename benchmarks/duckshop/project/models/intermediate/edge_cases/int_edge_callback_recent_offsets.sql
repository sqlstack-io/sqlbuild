MODEL (
  description """
    One row per gateway callback on a business date up to today and after 1 March 2026, the
    zoned time compared with date text
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    event_at (type TIMESTAMP_TZ),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    g.event_at
  FROM __ref("stg_edge_gateway_callbacks") AS g
  WHERE
    g.business_date <= CURRENT_DATE()
    AND g.event_at >= '2026-03-01'
)

SELECT *
FROM final
