MODEL (
  description """
    One row per category reached by a recursive CTE written without the RECURSIVE keyword, which
    Snowflake does not require. Its columns shift by one step per iteration, so the grandparent's
    name reaches a category only after two iterations
  """,
  tags [edge_case],
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    tree_depth (type NUMBER(38, 0)),
    parent_name (type VARCHAR),
    grandparent_name (type VARCHAR),
  ),
);

WITH links AS (
  SELECT
    l.category_id,
    l.parent_id,
    l.category_name
  FROM __ref("stg_edge_category_links") AS l
),

tree(category_id, tree_depth, own_name, parent_name, grandparent_name) AS (
  SELECT
    l.category_id,
    1::NUMBER(38, 0) AS tree_depth,
    l.category_name::VARCHAR AS own_name,
    NULL::VARCHAR AS parent_name,
    NULL::VARCHAR AS grandparent_name
  FROM links AS l
  WHERE l.parent_id IS NULL
  UNION ALL
  SELECT
    l.category_id,
    (
      t.tree_depth + 1
    )::NUMBER(38, 0) AS tree_depth,
    l.category_name::VARCHAR AS own_name,
    t.own_name::VARCHAR AS parent_name,
    t.parent_name::VARCHAR AS grandparent_name
  FROM links AS l
  INNER JOIN tree AS t
    ON l.parent_id = t.category_id
),

final AS (
  SELECT
    t.category_id,
    t.tree_depth::NUMBER(38, 0) AS tree_depth,
    t.parent_name::VARCHAR AS parent_name,
    t.grandparent_name::VARCHAR AS grandparent_name
  FROM tree AS t
)

SELECT *
FROM final
