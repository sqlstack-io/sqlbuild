MODEL (
  description """
    One row per merchant zone with the number of day documents, objects holding a DATE,
    aggregated from its callbacks
  """,
  tags [edge_case],
  columns (
    merchant_zone (type VARCHAR, nullable false, audits [not_null, unique]),
    document_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    g.merchant_zone,
    ARRAY_SIZE(ARRAY_AGG(OBJECT_CONSTRUCT_KEEP_NULL('day', g.business_date, 'id', g.callback_id)))::NUMBER(38, 0)
      AS document_count
  FROM __ref("stg_edge_gateway_callbacks") AS g
  GROUP BY
    g.merchant_zone
)

SELECT *
FROM final
