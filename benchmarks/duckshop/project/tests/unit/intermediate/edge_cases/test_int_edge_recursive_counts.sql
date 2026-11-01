TEST (name "int_edge_recursive_counts__recurses_without_the_recursive_keyword");

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

__expected__int_edge_recursive_counts AS (
  SELECT
    NULL AS category_id,
    NULL AS tree_depth,
    NULL AS parent_name,
    NULL AS grandparent_name
)
