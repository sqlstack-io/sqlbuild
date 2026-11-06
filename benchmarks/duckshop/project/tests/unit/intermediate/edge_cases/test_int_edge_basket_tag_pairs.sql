TEST (name "int_edge_basket_tag_pairs__keeps_pairs_in_different_orders_apart");

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

__expected__int_edge_basket_tag_pairs AS (
  SELECT
    'bath|bath' AS tag_pair_text,
    PARSE_JSON('["bath","bath"]')::ARRAY AS tag_pair,
    2 AS pair_size
  UNION ALL
  SELECT
    'bath|garden' AS tag_pair_text,
    PARSE_JSON('["bath","garden"]')::ARRAY AS tag_pair,
    2 AS pair_size
  UNION ALL
  SELECT
    'bath|gift' AS tag_pair_text,
    PARSE_JSON('["bath","gift"]')::ARRAY AS tag_pair,
    2 AS pair_size
  UNION ALL
  SELECT
    'garden|bath' AS tag_pair_text,
    PARSE_JSON('["garden","bath"]')::ARRAY AS tag_pair,
    2 AS pair_size
  UNION ALL
  SELECT
    'gift|' AS tag_pair_text,
    PARSE_JSON('["gift",undefined]')::ARRAY AS tag_pair,
    2 AS pair_size
  UNION ALL
  SELECT
    'gift|bath' AS tag_pair_text,
    PARSE_JSON('["gift","bath"]')::ARRAY AS tag_pair,
    2 AS pair_size
  UNION ALL
  SELECT
    '|gift' AS tag_pair_text,
    PARSE_JSON('[undefined,"gift"]')::ARRAY AS tag_pair,
    2 AS pair_size
)
