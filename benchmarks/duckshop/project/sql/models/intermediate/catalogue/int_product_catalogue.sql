MODEL (
  description """
    One row per sellable variant with its product, brand, category path, primary image and flattened
    specs: the catalogue as customers see it.
  """,
  columns (
    sku (type VARCHAR, nullable false, audits [not_null, unique]),
    product_id (type VARCHAR, audits [not_null]),
    sku_family (type VARCHAR),
    title (type VARCHAR),
    variant_label (type VARCHAR),
    brand_code (type VARCHAR),
    brand_name (type VARCHAR),
    is_private_label (type BOOLEAN),
    category_id (type VARCHAR),
    category_slug_path (type VARCHAR),
    category_level_1 (type VARCHAR),
    material (type VARCHAR),
    weight_kg (type NUMBER(10, 3)),
    primary_image_url (type VARCHAR),
    image_count (type INT),
    is_active (type BOOLEAN),
    launched_on (type DATE),
    is_bundle_component (type BOOLEAN),
  ),
);

WITH variants AS (
  SELECT *
  FROM __ref("stg_product_variants")
),

products AS (
  SELECT *
  FROM __ref("stg_products")
),

brands AS (
  SELECT *
  FROM __ref("stg_brands")
),

category_paths AS (
  SELECT *
  FROM __ref("int_category_paths")
),

media AS (
  SELECT *
  FROM __ref("stg_product_media")
),

specs AS (
  SELECT *
  FROM __ref("int_product_specs_flat")
),

bundle_components AS (
  SELECT *
  FROM __ref("stg_bundle_components")
),

leaf_categories AS (
  SELECT
    cp.category_id,
    cp.slug_path
  FROM category_paths AS cp
),

images AS (
  SELECT
    m.product_id,
    MIN_BY(m.media_url, m.position) AS primary_image_url,
    COUNT(*) AS image_count
  FROM media AS m
  WHERE m.media_type = 'image'
  GROUP BY
    m.product_id
),

component_skus AS (
  SELECT DISTINCT bc.component_sku
  FROM bundle_components AS bc
),

final AS (
  SELECT
    v.sku,
    v.product_id,
    __udf("udf__sku_family")(v.sku)::VARCHAR AS sku_family,
    p.title,
    NULLIF(
      CONCAT_WS(' / ', v.colour, v.size_label),
      ''
    )::VARCHAR AS variant_label,
    p.brand_code,
    b.brand_name,
    COALESCE(b.is_private_label, FALSE) AS is_private_label,
    lc.category_id,
    COALESCE(lc.slug_path, p.category_path)::VARCHAR AS category_slug_path,
    p.category_level_1,
    s.material,
    v.weight_kg,
    i.primary_image_url::VARCHAR AS primary_image_url,
    COALESCE(i.image_count, 0)::INT AS image_count,
    p.is_active,
    p.launched_on,
    (
      cs.component_sku IS NOT NULL
    ) AS is_bundle_component
  FROM variants AS v
  INNER JOIN products AS p
    ON v.product_id = p.product_id
  LEFT JOIN brands AS b
    ON p.brand_code = b.brand_code
  LEFT JOIN leaf_categories AS lc
    ON p.category_path = lc.slug_path
  LEFT JOIN images AS i
    ON v.product_id = i.product_id
  LEFT JOIN specs AS s
    ON v.product_id = s.product_id
  LEFT JOIN component_skus AS cs
    ON v.sku = cs.component_sku
)

SELECT *
FROM final
