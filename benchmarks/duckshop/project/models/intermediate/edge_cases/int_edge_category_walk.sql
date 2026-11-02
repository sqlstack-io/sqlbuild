MODEL (
  description """
    One row per category reached by walking the category tree with START WITH ... CONNECT BY, with
    its walk depth (the LEVEL pseudo-column), root name and path. A second walk joins in a relation
    with a column named level: there Snowflake reads even the qualified d.level as the walk depth,
    and CONNECT_BY_ROOT returns the row's own value instead of its root's. After the walk, a scalar
    subquery reads the level column itself. Snowflake fails with an internal error on a scalar
    subquery inside a CONNECT BY query, so none is written there
  """,
  tags [edge_case],
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    walk_depth (type NUMBER(38, 0)),
    root_name (type VARCHAR),
    category_path (type VARCHAR),
    joined_level (type NUMBER(38, 0)),
    joined_root_name (type VARCHAR),
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

deepest AS (
  SELECT MAX(s.level) AS level
  FROM shelves AS s
),

walk AS (
  SELECT
    k.category_id,
    LEVEL AS walk_depth,
    CONNECT_BY_ROOT k.category_name AS root_name,
    SYS_CONNECT_BY_PATH(k.category_name, ' > ') AS category_path
  FROM links AS k
START WITH k.parent_id IS NULL
CONNECT BY k.parent_id = PRIOR k.category_id
),

joined_walk AS (
  SELECT
    k.category_id,
    d.level AS joined_level,
    CONNECT_BY_ROOT k.category_name AS joined_root_name
  FROM links AS k
  CROSS JOIN deepest AS d
START WITH k.parent_id IS NULL
CONNECT BY k.parent_id = PRIOR k.category_id
),

final AS (
  SELECT
    w.category_id,
    w.walk_depth::NUMBER(38, 0) AS walk_depth,
    w.root_name::VARCHAR AS root_name,
    w.category_path::VARCHAR AS category_path,
    j.joined_level::NUMBER(38, 0) AS joined_level,
    j.joined_root_name::VARCHAR AS joined_root_name,
    (
      SELECT MAX(level)
      FROM shelves
    )::NUMBER(38, 0) AS deepest_shelf
  FROM walk AS w
  INNER JOIN joined_walk AS j
    ON w.category_id = j.category_id
)

SELECT *
FROM final
