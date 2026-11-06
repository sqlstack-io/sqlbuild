TEST (name "int_edge_basket_top_tags__ranks_arrays_by_position_and_name");

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

__expected__int_edge_basket_top_tags AS (
  SELECT
    'by_name' AS pick_kind,
    'B-3' AS picked_id,
    CAST(NULL AS NUMBER(38, 0)) AS picked_size
  UNION ALL
  SELECT
    'by_position' AS pick_kind,
    'B-3' AS picked_id,
    1 AS picked_size
  UNION ALL
  SELECT
    'constructed' AS pick_kind,
    'BL-03' AS picked_id,
    1 AS picked_size
  UNION ALL
  SELECT
    'over_union' AS pick_kind,
    'B-3' AS picked_id,
    1 AS picked_size
  UNION ALL
  SELECT
    'through_iff' AS pick_kind,
    'B-3' AS picked_id,
    1 AS picked_size
)
