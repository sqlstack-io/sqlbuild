TEST (name "int_edge_basket_tag_list_reads__reads_tag_lists");

WITH __ref__stg_edge_basket_lines AS (
  SELECT
    v.line_id::VARCHAR AS line_id,
    v.basket_id::VARCHAR AS basket_id,
    v.line_no::NUMBER(38, 0) AS line_no,
    v.sku::VARCHAR AS sku,
    v.tag::VARCHAR AS tag
  FROM VALUES
    ('BL-01', 'B-1', 1, 'DK-RED-01', 'red'),
    ('BL-02', 'B-1', 2, 'DK-BLUE-02', 'blue'),
    ('BL-03', 'B-1', 3, 'DK-RED-03', 'red'),
    ('BL-04', 'B-2', 1, 'TL-GREEN-01', NULL) AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__int_edge_basket_tag_list_reads AS (
  SELECT
    'B-1' AS basket_id,
    TRUE AS has_colour,
    2 AS distinct_tag_count,
    0 AS red_position,
    1 AS non_red_count,
    'RED,BLUE,RED' AS upper_tags,
    CAST(NULL AS TIME(9)) AS basket_time,
    2 AS pair_size
  UNION ALL
  SELECT
    'B-2' AS basket_id,
    FALSE AS has_colour,
    0 AS distinct_tag_count,
    CAST(NULL AS NUMBER(38,0)) AS red_position,
    0 AS non_red_count,
    '' AS upper_tags,
    CAST(NULL AS TIME(9)) AS basket_time,
    2 AS pair_size
)
