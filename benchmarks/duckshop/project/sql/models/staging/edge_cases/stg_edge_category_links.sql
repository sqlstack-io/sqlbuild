MODEL (
  description """
    One row per catalogue category link, with the shelf depth each category is displayed at, for
    walking the category tree with CONNECT BY and a recursive CTE
  """,
  tags [edge_case],
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    parent_id (type VARCHAR),
    category_name (type VARCHAR),
    shelf_depth (type NUMBER(38, 0)),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__category_links")
),

final AS (
  SELECT
    s.category_id,
    s.parent_id,
    s.category_name,
    s.shelf_depth
  FROM src AS s
)

SELECT *
FROM final
