MODEL (
  description """
    One row per category node with its depth, its full slug path from the root and its root
    category, walked with CONNECT BY over the parent links. Snowflake has no CONNECT_BY_ISLEAF, so a
    node is a leaf when no category names it as a parent.
  """,
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    parent_category_id (type VARCHAR),
    root_category_id (type VARCHAR),
    slug (type VARCHAR),
    display_name (type VARCHAR),
    depth (type INT),
    slug_path (type VARCHAR),
    is_leaf (type BOOLEAN),
    sibling_rank (type INT),
  ),
);

WITH categories AS (
  SELECT *
  FROM __ref("stg_categories")
),

tree AS (
  SELECT
    c.category_id,
    c.parent_category_id,
    c.slug,
    c.display_name,
    c.sort_order,
    LEVEL AS depth,
    SYS_CONNECT_BY_PATH(c.slug, '/') AS slug_path,
    CONNECT_BY_ROOT c.category_id AS root_category_id
  FROM categories AS c
START WITH c.is_root
CONNECT BY PRIOR c.category_id = c.parent_category_id
),

parents AS (
  SELECT DISTINCT
    c.parent_category_id
  FROM categories AS c
  WHERE c.parent_category_id IS NOT NULL
),

final AS (
  SELECT
    t.category_id,
    t.parent_category_id,
    t.root_category_id::VARCHAR AS root_category_id,
    t.slug,
    t.display_name,
    t.depth::INT AS depth,
    LTRIM(t.slug_path, '/')::VARCHAR AS slug_path,
    (
      p.parent_category_id IS NULL
    ) AS is_leaf,
    RANK() OVER (
      PARTITION BY t.parent_category_id
      ORDER BY t.sort_order, t.slug
    )::INT AS sibling_rank
  FROM tree AS t
  LEFT JOIN parents AS p
    ON t.category_id = p.parent_category_id
)

SELECT *
FROM final
