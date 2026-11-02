TEST (name "int_edge_basket_xml_reads__reads_elements_back_from_line_documents");

WITH __ref__stg_edge_basket_lines AS (
  SELECT
    v.line_id,
    v.basket_id,
    v.line_no::NUMBER(38, 0) AS line_no,
    v.sku,
    v.tag
  FROM VALUES
    ('BL-01', 'B-1', 1, 'MALLARD-01', 'bath'),
    ('BL-02', 'B-1', 2, 'TEAL-02', NULL),
    ('BL-03', 'B-2', 1, 'EIDER-03', 'gift wrap') AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__int_edge_basket_xml_reads AS (
  SELECT
    'BL-01' AS line_id,
    TRUE AS document_is_valid,
    FALSE AS broken_copy_is_valid,
    'line' AS root_name,
    'bath' AS tag_text,
    'MALLARD-01' AS sku_text
  UNION ALL
  SELECT
    'BL-02' AS line_id,
    TRUE AS document_is_valid,
    FALSE AS broken_copy_is_valid,
    'line' AS root_name,
    '' AS tag_text,
    'TEAL-02' AS sku_text
  UNION ALL
  SELECT
    'BL-03' AS line_id,
    TRUE AS document_is_valid,
    FALSE AS broken_copy_is_valid,
    'line' AS root_name,
    'gift wrap' AS tag_text,
    'EIDER-03' AS sku_text
)
