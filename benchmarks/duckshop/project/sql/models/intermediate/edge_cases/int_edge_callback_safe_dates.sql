MODEL (
  description """
    One row per gateway callback with its zone and year in an object that keeps NULL values, and
    its business date passed through TRY_CAST to DATE
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    callback_json (type VARCHAR),
    business_date_again (type DATE),
  ),
);

WITH callback_objects AS (
  SELECT
    g.callback_id,
    OBJECT_CONSTRUCT_KEEP_NULL('zone', g.merchant_zone, 'year', YEAR(g.business_date)) AS callback_object,
    g.business_date
  FROM __ref("stg_edge_gateway_callbacks") AS g
),

final AS (
  SELECT
    o.callback_id,
    TO_JSON(o.callback_object) AS callback_json,
    TRY_CAST(o.business_date AS DATE) AS business_date_again
  FROM callback_objects AS o
)

SELECT *
FROM final
