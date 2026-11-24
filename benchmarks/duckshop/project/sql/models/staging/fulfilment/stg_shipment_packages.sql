MODEL (
  description "One row per live shipment package with volumetric weight",
  columns (
    package_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    weight_grams (type INT),
    length_cm (type NUMBER(8, 2)),
    width_cm (type NUMBER(8, 2)),
    height_cm (type NUMBER(8, 2)),
    volumetric_weight_kg (type NUMBER(10, 3)),
    content_line_count (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__shipment_packages")
),

@sql.cdc_latest(src, src.key := (package_id))

final AS (
  SELECT
    p.package_id,
    p.shipment_id,
    p.weight_grams::INT AS weight_grams,
    p.dimensions:l::NUMBER(8, 2) AS length_cm,
    p.dimensions:w::NUMBER(8, 2) AS width_cm,
    p.dimensions:h::NUMBER(8, 2) AS height_cm,
    ROUND(
      p.dimensions:l::NUMBER(8, 2) * p.dimensions:w::NUMBER(8, 2) * p.dimensions:h::NUMBER(8, 2)
        / 5000,
      3
    )::NUMBER(10, 3) AS volumetric_weight_kg,
    ARRAY_SIZE(COALESCE(p.contents, ARRAY_CONSTRUCT()))::INT AS content_line_count,
    p.contents::ARRAY AS contents,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
