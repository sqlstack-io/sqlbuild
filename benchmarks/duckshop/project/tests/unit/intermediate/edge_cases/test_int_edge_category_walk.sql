TEST (name "int_edge_category_walk__reads_a_level_column_beside_the_walk_depth");

WITH __ref__stg_edge_category_links AS (
  SELECT
    v.category_id,
    v.parent_id,
    v.category_name,
    v.shelf_depth::NUMBER(38, 0) AS shelf_depth
  FROM VALUES
    ('CAT-1', NULL, 'Ducks', 1),
    ('CAT-2', 'CAT-1', 'Rubber ducks', 4),
    ('CAT-3', 'CAT-2', 'Collectible series', 7),
    ('CAT-4', 'CAT-1', 'Decoys', 4),
    ('CAT-5', NULL, 'Pond supplies', 1) AS v(category_id, parent_id, category_name, shelf_depth)
),

__expected__int_edge_category_walk AS (
  SELECT
    'CAT-1' AS category_id,
    1 AS walk_depth,
    'Ducks' AS root_name,
    ' > Ducks' AS category_path,
    1 AS joined_level,
    'Ducks' AS joined_root_name,
    7 AS deepest_shelf
  UNION ALL
  SELECT
    'CAT-2' AS category_id,
    2 AS walk_depth,
    'Ducks' AS root_name,
    ' > Ducks > Rubber ducks' AS category_path,
    2 AS joined_level,
    'Rubber ducks' AS joined_root_name,
    7 AS deepest_shelf
  UNION ALL
  SELECT
    'CAT-3' AS category_id,
    3 AS walk_depth,
    'Ducks' AS root_name,
    ' > Ducks > Rubber ducks > Collectible series' AS category_path,
    3 AS joined_level,
    'Collectible series' AS joined_root_name,
    7 AS deepest_shelf
  UNION ALL
  SELECT
    'CAT-4' AS category_id,
    2 AS walk_depth,
    'Ducks' AS root_name,
    ' > Ducks > Decoys' AS category_path,
    2 AS joined_level,
    'Decoys' AS joined_root_name,
    7 AS deepest_shelf
  UNION ALL
  SELECT
    'CAT-5' AS category_id,
    1 AS walk_depth,
    'Pond supplies' AS root_name,
    ' > Pond supplies' AS category_path,
    1 AS joined_level,
    'Pond supplies' AS joined_root_name,
    7 AS deepest_shelf
)
