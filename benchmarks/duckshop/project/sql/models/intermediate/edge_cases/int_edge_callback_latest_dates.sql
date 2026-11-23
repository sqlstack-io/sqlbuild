MODEL (
  description """
    One row per merchant zone with the business date, held as VARIANT, of its latest callback
  """,
  tags [edge_case],
  columns (
    merchant_zone (type VARCHAR, nullable false, audits [not_null, unique]),
    latest_date_text (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    g.merchant_zone,
    MAX_BY(TO_VARIANT(g.business_date), g.callback_id)::VARCHAR AS latest_date_text
  FROM __ref("stg_edge_gateway_callbacks") AS g
  GROUP BY
    g.merchant_zone
)

SELECT *
FROM final
