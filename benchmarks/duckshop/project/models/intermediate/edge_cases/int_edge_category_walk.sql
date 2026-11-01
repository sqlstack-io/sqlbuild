MODEL (
  description """
    One row per category reached by walking the category tree with START WITH ... CONNECT BY, with
    its walk depth (the LEVEL pseudo-column), root name and path. Beside it, a scalar subquery reads
    a column named level from another relation: inside the subquery that name is the column, not the
    walk depth
  """,
  tags [edge_case],
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    walk_depth (type NUMBER(38, 0)),
    root_name (type VARCHAR),
    category_path (type VARCHAR),
    deepest_shelf (type NUMBER(38, 0)),
  ),
);

WITH links AS (
  SELECT
    l.category_id,
    l.parent_id,
    l.category_name
  FROM __ref("stg_edge_category_links") AS l
),

shelves AS (
  SELECT l.shelf_depth AS level
  FROM __ref("stg_edge_category_links") AS l
),

walk AS (
  SELECT
    k.category_id,
    LEVEL AS walk_depth,
    CONNECT_BY_ROOT k.category_name AS root_name,
    SYS_CONNECT_BY_PATH(k.category_name, ' > ') AS category_path,
    (
      SELECT MAX(level)
      FROM shelves
    ) AS deepest_shelf
  FROM links AS k
START WITH k.parent_id IS NULL
CONNECT BY k.parent_id = PRIOR k.category_id
),

final AS (
  SELECT
    w.category_id,
    w.walk_depth::NUMBER(38, 0) AS walk_depth,
    w.root_name::VARCHAR AS root_name,
    w.category_path::VARCHAR AS category_path,
    w.deepest_shelf::NUMBER(38, 0) AS deepest_shelf
  FROM walk AS w
)

SELECT *
FROM final
