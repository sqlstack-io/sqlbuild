MODEL (
  description """
    One row per merchant zone with the distinct business dates of its callbacks, held as VARIANT
  """,
  tags [edge_case],
  columns (
    merchant_zone (type VARCHAR, nullable false, audits [not_null, unique]),
    date_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    g.merchant_zone,
    ARRAY_SIZE(ARRAY_AGG(DISTINCT TO_VARIANT(g.business_date)))::NUMBER(38, 0) AS date_count
  FROM __ref("stg_edge_gateway_callbacks") AS g
  GROUP BY
    g.merchant_zone
)

SELECT *
FROM final
