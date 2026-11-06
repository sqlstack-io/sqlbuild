TEST (name "stg_edge_category_links__passes_links_through");

WITH __source__edge__category_links AS (
  SELECT
    v.category_id,
    v.parent_id,
    v.category_name,
    v.shelf_depth::NUMBER(38, 0) AS shelf_depth,
    '2026-03-06 00:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('CAT-1', NULL, 'Ducks', 1),
    ('CAT-2', 'CAT-1', 'Rubber ducks', 2) AS v(category_id, parent_id, category_name, shelf_depth)
),

__expected__stg_edge_category_links AS (
  SELECT
    'CAT-1' AS category_id,
    CAST(NULL AS VARCHAR) AS parent_id,
    'Ducks' AS category_name,
    1 AS shelf_depth
  UNION ALL
  SELECT
    'CAT-2' AS category_id,
    'CAT-1' AS parent_id,
    'Rubber ducks' AS category_name,
    2 AS shelf_depth
)
