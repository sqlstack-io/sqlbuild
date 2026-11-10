TEST (name "int_edge_basket_line_laterals__reads_the_table_function_once_per_basket");

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
    ('BL-03', 'B-2', 1, 'EIDER-03', NULL) AS v(line_id, basket_id, line_no, sku, tag)
),

__table_fn__edge_basket_lines_for AS (
  SELECT
    v.line_id::VARCHAR AS line_id,
    v.line_no::INT AS line_no,
    v.sku::VARCHAR AS sku,
    v.tag::VARCHAR AS tag
  FROM VALUES ('BL-01', 1, 'MALLARD-01', 'bath') AS v(line_id, line_no, sku, tag)
),

__expected__int_edge_basket_line_laterals AS (
  SELECT
    'B-1' AS basket_id,
    'BL-01' AS line_id,
    'MALLARD-01' AS sku
  UNION ALL
  SELECT
    'B-2' AS basket_id,
    'BL-01' AS line_id,
    'MALLARD-01' AS sku
)
