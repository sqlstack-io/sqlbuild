TEST (name "int_edge_basket_array_edits__appends_intersects_and_slices_sku_lists");

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
    ('BL-05', 'B-3', 1, 'MERGANSER-05', NULL),
    ('BL-06', 'B-3', 2, 'EIDER-03', 'gift') AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__int_edge_basket_array_edits AS (
  SELECT
    'B-1' AS basket_id,
    PARSE_JSON('["MALLARD-01","TEAL-02","EIDER-03"]')::ARRAY AS skus,
    PARSE_JSON('["MALLARD-01","TEAL-02","EIDER-03","BAG-00"]')::ARRAY AS skus_with_bag,
    PARSE_JSON('["EIDER-03","MALLARD-01"]')::ARRAY AS common_skus,
    PARSE_JSON('["MALLARD-01","TEAL-02"]')::ARRAY AS first_two_skus,
    PARSE_JSON('["MALLARD-01","TEAL-02"]')::ARRAY AS all_but_last_sku,
    PARSE_JSON('["bath"]')::ARRAY AS first_tag_array,
    CAST(NULL AS ARRAY) AS missing_tag_array,
    PARSE_JSON('["B-1",3]')::ARRAY AS bracket_pair
  UNION ALL
  SELECT
    'B-2' AS basket_id,
    PARSE_JSON('["PINTAIL-04"]')::ARRAY AS skus,
    PARSE_JSON('["PINTAIL-04","BAG-00"]')::ARRAY AS skus_with_bag,
    PARSE_JSON('[]')::ARRAY AS common_skus,
    PARSE_JSON('["PINTAIL-04"]')::ARRAY AS first_two_skus,
    PARSE_JSON('[]')::ARRAY AS all_but_last_sku,
    PARSE_JSON('["garden"]')::ARRAY AS first_tag_array,
    CAST(NULL AS ARRAY) AS missing_tag_array,
    PARSE_JSON('["B-2",1]')::ARRAY AS bracket_pair
  UNION ALL
  SELECT
    'B-3' AS basket_id,
    PARSE_JSON('["MERGANSER-05","EIDER-03"]')::ARRAY AS skus,
    PARSE_JSON('["MERGANSER-05","EIDER-03","BAG-00"]')::ARRAY AS skus_with_bag,
    PARSE_JSON('["EIDER-03"]')::ARRAY AS common_skus,
    PARSE_JSON('["MERGANSER-05","EIDER-03"]')::ARRAY AS first_two_skus,
    PARSE_JSON('["MERGANSER-05"]')::ARRAY AS all_but_last_sku,
    CAST(NULL AS ARRAY) AS first_tag_array,
    CAST(NULL AS ARRAY) AS missing_tag_array,
    PARSE_JSON('["B-3",2]')::ARRAY AS bracket_pair
)
