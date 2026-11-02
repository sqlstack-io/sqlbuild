TEST (name "int_edge_basket_line_lookups__reads_one_basket_through_the_table_function");

WITH __table_fn__edge_basket_lines_for AS (
  SELECT
    v.line_id::VARCHAR AS line_id,
    v.line_no::INT AS line_no,
    v.sku::VARCHAR AS sku,
    v.tag::VARCHAR AS tag
  FROM VALUES
    ('BL-01', 1, 'DK-RED-01', 'red'),
    ('BL-02', 2, 'DK-BLUE-02', NULL) AS v(line_id, line_no, sku, tag)
),

__expected__int_edge_basket_line_lookups AS (
  SELECT
    'BL-01' AS line_id,
    1 AS line_no,
    'DK-RED-01' AS sku
  UNION ALL
  SELECT
    'BL-02' AS line_id,
    2 AS line_no,
    'DK-BLUE-02' AS sku
)
