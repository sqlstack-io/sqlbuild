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
    NULL AS basket_id,
    NULL AS tag_count,
    NULL AS has_gift,
    NULL AS overlaps_pond_or_garden,
    NULL AS distinct_tag_count,
    NULL AS distinct_tags,
    NULL AS flattened_count,
    NULL AS flattened_tags
)
