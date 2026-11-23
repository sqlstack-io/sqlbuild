MODEL (
  description """
    Version history of catalogue products, recording a new version when merchandising attributes
    change
  """,
  materialized snapshot,
  unique_key [product_id],
  snapshot_strategy check,
  check_columns [title, brand_code, category_path, is_active],
  columns (
    product_id (type VARCHAR, nullable false, audits [not_null]),
    title (type VARCHAR),
    brand_code (type VARCHAR),
    category_path (type VARCHAR),
    is_active (type BOOLEAN),
    spec_key_count (type INT),
  ),
);

WITH products AS (
  SELECT *
  FROM __ref("stg_products")
),

final AS (
  SELECT
    p.product_id,
    p.title,
    p.brand_code,
    p.category_path,
    p.is_active,
    p.spec_key_count
  FROM products AS p
)

SELECT *
FROM final
