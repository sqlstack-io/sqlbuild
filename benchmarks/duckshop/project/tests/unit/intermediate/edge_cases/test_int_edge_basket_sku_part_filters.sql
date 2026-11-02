TEST (name "int_edge_basket_sku_part_filters__counts_long_sku_parts");

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

__expected__int_edge_basket_sku_part_filters AS (
  SELECT
    NULL AS line_id,
    NULL AS long_part_count
)
