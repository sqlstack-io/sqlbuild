TEST (name "int_edge_sku_text_reads__reads_skus_with_text_and_encoding_functions");

WITH __ref__stg_edge_basket_lines AS (
  SELECT
    v.line_id,
    v.basket_id,
    v.line_no::NUMBER(38, 0) AS line_no,
    v.sku,
    v.tag
  FROM VALUES
    ('BL-01', 'B-1', 1, 'MALLARD-01', 'gift'),
    ('BL-02', 'B-1', 2, 'TEAL-02', '6869'),
    ('BL-03', 'B-2', 1, 'EIDER-1-B', 'Z2lmdA=='),
    ('BL-04', 'B-2', 2, 'SWAN', 'abc'),
    ('BL-05', 'B-3', 1, 'MALLARD-11', NULL) AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__int_edge_sku_text_reads AS (
  SELECT
    'BL-01' AS line_id,
    TRUE AS is_mallard,
    TRUE AS ends_in_one,
    'MALLARD' AS sku_family,
    8 AS dash_position,
    8 AS dash_position_comma,
    8 AS second_dash_position,
    'MALLARD-01#' AS sku_with_marker,
    PARSE_JSON('["0","1"]')::ARRAY AS sku_digits,
    '1' AS line_no_text,
    '4D414C4C4152442D3031' AS sku_hex,
    'MALLARD-01' AS sku_from_hex,
    CAST(NULL AS VARCHAR) AS odd_hex_text,
    '8227ED' AS tag_base64_hex,
    TRUE AS is_gift_line
  UNION ALL
  SELECT
    'BL-02' AS line_id,
    FALSE AS is_mallard,
    FALSE AS ends_in_one,
    'TEAL' AS sku_family,
    5 AS dash_position,
    5 AS dash_position_comma,
    5 AS second_dash_position,
    'TEAL-02#' AS sku_with_marker,
    PARSE_JSON('["0","2"]')::ARRAY AS sku_digits,
    '2' AS line_no_text,
    '5445414C2D3032' AS sku_hex,
    'TEAL-02' AS sku_from_hex,
    'hi' AS odd_hex_text,
    'EBCEBD' AS tag_base64_hex,
    FALSE AS is_gift_line
  UNION ALL
  SELECT
    'BL-03' AS line_id,
    FALSE AS is_mallard,
    FALSE AS ends_in_one,
    'EIDER' AS sku_family,
    6 AS dash_position,
    6 AS dash_position_comma,
    6 AS second_dash_position,
    'EIDER-1-B#' AS sku_with_marker,
    PARSE_JSON('["1"]')::ARRAY AS sku_digits,
    '1' AS line_no_text,
    '45494445522D312D42' AS sku_hex,
    'EIDER-1-B' AS sku_from_hex,
    CAST(NULL AS VARCHAR) AS odd_hex_text,
    '67696674' AS tag_base64_hex,
    FALSE AS is_gift_line
  UNION ALL
  SELECT
    'BL-04' AS line_id,
    FALSE AS is_mallard,
    FALSE AS ends_in_one,
    '' AS sku_family,
    0 AS dash_position,
    0 AS dash_position_comma,
    5 AS second_dash_position,
    'SWAN#' AS sku_with_marker,
    PARSE_JSON('[]')::ARRAY AS sku_digits,
    '2' AS line_no_text,
    '5357414E' AS sku_hex,
    'SWAN' AS sku_from_hex,
    CAST(NULL AS VARCHAR) AS odd_hex_text,
    CAST(NULL AS VARCHAR) AS tag_base64_hex,
    FALSE AS is_gift_line
  UNION ALL
  SELECT
    'BL-05' AS line_id,
    TRUE AS is_mallard,
    TRUE AS ends_in_one,
    'MALLARD' AS sku_family,
    8 AS dash_position,
    8 AS dash_position_comma,
    8 AS second_dash_position,
    'MALLARD-11#' AS sku_with_marker,
    PARSE_JSON('["1","1"]')::ARRAY AS sku_digits,
    '1' AS line_no_text,
    '4D414C4C4152442D3131' AS sku_hex,
    'MALLARD-11' AS sku_from_hex,
    CAST(NULL AS VARCHAR) AS odd_hex_text,
    CAST(NULL AS VARCHAR) AS tag_base64_hex,
    CAST(NULL AS BOOLEAN) AS is_gift_line
)
