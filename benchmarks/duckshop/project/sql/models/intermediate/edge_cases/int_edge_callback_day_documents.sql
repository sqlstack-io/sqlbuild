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

WITH day_documents AS (
  SELECT
    g.merchant_zone,
    ARRAY_AGG(OBJECT_CONSTRUCT_KEEP_NULL('id', g.callback_id, 'day', g.business_date)) AS documents
  FROM __ref("stg_edge_gateway_callbacks") AS g
  GROUP BY
    g.merchant_zone
),

final AS (
  SELECT
    d.merchant_zone,
    ARRAY_SIZE(d.documents)::NUMBER(38, 0) AS document_count
  FROM day_documents AS d
)

SELECT *
FROM final
