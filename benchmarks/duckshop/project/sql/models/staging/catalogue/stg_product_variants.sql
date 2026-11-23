MODEL (
  description "One row per live sellable variant (SKU)",
  columns (
    sku (type VARCHAR, nullable false, audits [not_null, unique]),
    product_id (type VARCHAR, audits [not_null]),
    colour (type VARCHAR),
    size_label (type VARCHAR),
    weight_grams (type INT),
    weight_kg (type NUMBER(10, 3)),
    barcode_count (type INT),
    is_fragile (type BOOLEAN),
    pack_size (type INT),
    attributes (type OBJECT),
    barcodes (type ARRAY),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__product_variants")
),

@sql.cdc_latest(src, src.key := (sku))

final AS (
  SELECT
    @sql.normalise_sku(v.sku) AS sku,
    v.product_id,
    INITCAP(TRIM(v.colour))::VARCHAR AS colour,
    UPPER(REGEXP_REPLACE(v.size_label, '\\s+', ''))::VARCHAR AS size_label,
    v.weight_grams::INT AS weight_grams,
    ROUND(v.weight_grams / 1000, 3)::NUMBER(10, 3) AS weight_kg,
    ARRAY_SIZE(COALESCE(v.barcodes, ARRAY_CONSTRUCT()))::INT AS barcode_count,
    COALESCE(v.attributes:handling.fragile::BOOLEAN, FALSE) AS is_fragile,
    COALESCE(TRY_TO_NUMBER(v.attributes:pack_size::VARCHAR), 1)::INT AS pack_size,
    v.barcodes::ARRAY AS barcodes,
    v.attributes::OBJECT AS attributes,
    @sql.load_metadata(v)
  FROM cdc_latest AS v
)
SELECT *
FROM final
