MODEL (
  description "One row per live category node with its parent and English and German display names",
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    parent_category_id (type VARCHAR),
    slug (type VARCHAR, audits [not_null]),
    display_name (type VARCHAR),
    display_name_de (type VARCHAR),
    sort_order (type INT),
    translation_count (type INT),
    is_root (type BOOLEAN),
    translations (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__categories")
),

@sql.cdc_latest(src, src.key := (category_id))

final AS (
  SELECT
    c.category_id,
    NULLIF(c.parent_category_id, c.category_id)::VARCHAR AS parent_category_id,
    LOWER(REGEXP_REPLACE(TRIM(c.slug), '[^a-zA-Z0-9]+', '-'))::VARCHAR AS slug,
    COALESCE(c.translations:en::VARCHAR, c.display_name)::VARCHAR AS display_name,
    c.translations:de::VARCHAR AS display_name_de,
    COALESCE(c.sort_order, 999)::INT AS sort_order,
    ARRAY_SIZE(OBJECT_KEYS(COALESCE(c.translations, OBJECT_CONSTRUCT())))::INT AS translation_count,
    (
      c.parent_category_id IS NULL
      OR c.parent_category_id = c.category_id
    ) AS is_root,
    COALESCE(c.translations, OBJECT_CONSTRUCT())::OBJECT AS translations,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
