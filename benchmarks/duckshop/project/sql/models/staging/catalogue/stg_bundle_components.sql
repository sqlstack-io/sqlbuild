MODEL (
  description "One row per live component line of a bundle",
  columns (
    component_id (type VARCHAR, nullable false, audits [not_null, unique]),
    bundle_sku (type VARCHAR, audits [not_null]),
    component_sku (type VARCHAR, audits [not_null]),
    quantity (type INT),
    is_optional (type BOOLEAN),
    substitute_skus (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__bundle_components")
),

@sql.cdc_latest(src, src.key := (component_id))

final AS (
  SELECT
    c.component_id,
    @sql.normalise_sku(c.bundle_sku) AS bundle_sku,
    @sql.normalise_sku(c.component_sku) AS component_sku,
    GREATEST(COALESCE(c.quantity, 1), 1)::INT AS quantity,
    COALESCE(c.options:optional::BOOLEAN, FALSE) AS is_optional,
    COALESCE(c.options:substitutes, ARRAY_CONSTRUCT())::ARRAY AS substitute_skus,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
