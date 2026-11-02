TEST (name "int_edge_basket_tag_sets__reads_unordered_arrays_without_their_order");

WITH __ref__stg_edge_basket_lines AS (
  SELECT
    v.line_id,
    v.basket_id,
    v.line_no::NUMBER(38, 0) AS line_no,
    v.sku,
    v.tag
  FROM VALUES
    ('BL-01', 'B-1', 1, 'MALLARD-01', 'bath'),
    ('BL-02', 'B-1', 2, 'TEAL-02', 'gift'),
    ('BL-03', 'B-1', 3, 'EIDER-03', 'bath'),
    ('BL-04', 'B-2', 1, 'PINTAIL-04', 'garden'),
    ('BL-05', 'B-2', 2, 'MALLARD-01', 'bath'),
    ('BL-06', 'B-3', 1, 'MERGANSER-05', 'pond'),
    ('BL-07', 'B-4', 1, 'TEAL-02', NULL),
    ('BL-08', 'B-4', 2, 'EIDER-03', 'gift') AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__int_edge_basket_tag_sets AS (
  SELECT
    'B-1' AS basket_id,
    3 AS tag_count,
    TRUE AS has_gift,
    FALSE AS overlaps_pond_or_garden,
    2 AS distinct_tag_count,
    PARSE_JSON('["bath","gift"]')::ARRAY AS distinct_tags,
    3 AS flattened_count,
    'bath,gift' AS flattened_tags
  UNION ALL
  SELECT
    'B-2' AS basket_id,
    2 AS tag_count,
    FALSE AS has_gift,
    TRUE AS overlaps_pond_or_garden,
    3 AS distinct_tag_count,
    PARSE_JSON('["bath","garden","gift"]')::ARRAY AS distinct_tags,
    2 AS flattened_count,
    'bath,garden' AS flattened_tags
  UNION ALL
  SELECT
    'B-3' AS basket_id,
    1 AS tag_count,
    FALSE AS has_gift,
    TRUE AS overlaps_pond_or_garden,
    2 AS distinct_tag_count,
    PARSE_JSON('["gift","pond"]')::ARRAY AS distinct_tags,
    1 AS flattened_count,
    'pond' AS flattened_tags
  UNION ALL
  SELECT
    'B-4' AS basket_id,
    1 AS tag_count,
    TRUE AS has_gift,
    FALSE AS overlaps_pond_or_garden,
    1 AS distinct_tag_count,
    PARSE_JSON('["gift"]')::ARRAY AS distinct_tags,
    1 AS flattened_count,
    'gift' AS flattened_tags
)
