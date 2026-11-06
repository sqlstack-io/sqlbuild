MODEL (
  description """
    One row per live product with its category path split into levels and its specs object
  """,
  columns (
    product_id (type VARCHAR, nullable false, audits [not_null, unique]),
    title (type VARCHAR, audits [not_null]),
    category_path (type VARCHAR),
    category_level_1 (type VARCHAR),
    category_level_2 (type VARCHAR),
    category_leaf (type VARCHAR),
    category_depth (type INT),
    brand_code (type VARCHAR),
    launched_on (type DATE),
    is_active (type BOOLEAN),
    spec_key_count (type INT),
    specs (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__products")
),

@sql.cdc_latest(src, src.key := (product_id))

paths AS (
  SELECT
    p.product_id,
    p.title,
    p.brand_code,
    p.launched_on,
    p.is_active,
    p.specs,
    p._seq,
    p._loaded_at,
    LOWER(TRIM(REGEXP_REPLACE(p.category_path, '/{2,}', '/'), '/')) AS clean_path
  FROM cdc_latest AS p
), final AS (
  SELECT
    p.product_id,
    REGEXP_REPLACE(TRIM(p.title), '\\s{2,}', ' ')::VARCHAR AS title,
    p.clean_path::VARCHAR AS category_path,
    SPLIT_PART(p.clean_path, '/', 1)::VARCHAR AS category_level_1,
    NULLIF(SPLIT_PART(p.clean_path, '/', 2), '')::VARCHAR AS category_level_2,
    GET(
      SPLIT(p.clean_path, '/'),
      ARRAY_SIZE(SPLIT(p.clean_path, '/')) - 1
    )::VARCHAR AS category_leaf,
    ARRAY_SIZE(SPLIT(p.clean_path, '/'))::INT AS category_depth,
    UPPER(p.brand_code)::VARCHAR AS brand_code,
    p.launched_on,
    COALESCE(p.is_active, TRUE) AS is_active,
    ARRAY_SIZE(OBJECT_KEYS(p.specs))::INT AS spec_key_count,
    IFF(IS_OBJECT(p.specs), p.specs, OBJECT_CONSTRUCT())::OBJECT AS specs,
    @sql.load_metadata(p)
  FROM paths AS p
)
SELECT *
FROM final
