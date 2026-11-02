TEST (name "int_edge_row_deciders__picks_baskets_by_columns_it_never_outputs");

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

__expected__int_edge_row_deciders AS (
  SELECT
    'except_limit:B-4:-' AS decider_row,
    'except_limit' AS decider,
    'B-4' AS basket_id,
    CAST(NULL AS VARCHAR) AS detail
  UNION ALL
  SELECT
    'exists_filter:B-1:-' AS decider_row,
    'exists_filter' AS decider,
    'B-1' AS basket_id,
    CAST(NULL AS VARCHAR) AS detail
  UNION ALL
  SELECT
    'exists_filter:B-4:-' AS decider_row,
    'exists_filter' AS decider,
    'B-4' AS basket_id,
    CAST(NULL AS VARCHAR) AS detail
  UNION ALL
  SELECT
    'exists_output:B-1:true' AS decider_row,
    'exists_output' AS decider,
    'B-1' AS basket_id,
    'true' AS detail
  UNION ALL
  SELECT
    'exists_output:B-2:false' AS decider_row,
    'exists_output' AS decider,
    'B-2' AS basket_id,
    'false' AS detail
  UNION ALL
  SELECT
    'exists_output:B-3:false' AS decider_row,
    'exists_output' AS decider,
    'B-3' AS basket_id,
    'false' AS detail
  UNION ALL
  SELECT
    'exists_output:B-4:true' AS decider_row,
    'exists_output' AS decider,
    'B-4' AS basket_id,
    'true' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-1:BL-01#0' AS decider_row,
    'flatten_input' AS decider,
    'B-1' AS basket_id,
    'BL-01#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-1:BL-01#1' AS decider_row,
    'flatten_input' AS decider,
    'B-1' AS basket_id,
    'BL-01#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-1:BL-02#0' AS decider_row,
    'flatten_input' AS decider,
    'B-1' AS basket_id,
    'BL-02#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-1:BL-02#1' AS decider_row,
    'flatten_input' AS decider,
    'B-1' AS basket_id,
    'BL-02#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-1:BL-03#0' AS decider_row,
    'flatten_input' AS decider,
    'B-1' AS basket_id,
    'BL-03#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-1:BL-03#1' AS decider_row,
    'flatten_input' AS decider,
    'B-1' AS basket_id,
    'BL-03#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-2:BL-04#0' AS decider_row,
    'flatten_input' AS decider,
    'B-2' AS basket_id,
    'BL-04#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-2:BL-04#1' AS decider_row,
    'flatten_input' AS decider,
    'B-2' AS basket_id,
    'BL-04#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-2:BL-05#0' AS decider_row,
    'flatten_input' AS decider,
    'B-2' AS basket_id,
    'BL-05#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-2:BL-05#1' AS decider_row,
    'flatten_input' AS decider,
    'B-2' AS basket_id,
    'BL-05#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-3:BL-06#0' AS decider_row,
    'flatten_input' AS decider,
    'B-3' AS basket_id,
    'BL-06#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-3:BL-06#1' AS decider_row,
    'flatten_input' AS decider,
    'B-3' AS basket_id,
    'BL-06#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-4:BL-07#0' AS decider_row,
    'flatten_input' AS decider,
    'B-4' AS basket_id,
    'BL-07#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-4:BL-07#1' AS decider_row,
    'flatten_input' AS decider,
    'B-4' AS basket_id,
    'BL-07#1' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-4:BL-08#0' AS decider_row,
    'flatten_input' AS decider,
    'B-4' AS basket_id,
    'BL-08#0' AS detail
  UNION ALL
  SELECT
    'flatten_input:B-4:BL-08#1' AS decider_row,
    'flatten_input' AS decider,
    'B-4' AS basket_id,
    'BL-08#1' AS detail
  UNION ALL
  SELECT
    'group_by_all:B-1:3' AS decider_row,
    'group_by_all' AS decider,
    'B-1' AS basket_id,
    '3' AS detail
  UNION ALL
  SELECT
    'group_by_all:B-2:2' AS decider_row,
    'group_by_all' AS decider,
    'B-2' AS basket_id,
    '2' AS detail
  UNION ALL
  SELECT
    'group_by_all:B-3:1' AS decider_row,
    'group_by_all' AS decider,
    'B-3' AS basket_id,
    '1' AS detail
  UNION ALL
  SELECT
    'group_by_all:B-4:2' AS decider_row,
    'group_by_all' AS decider,
    'B-4' AS basket_id,
    '2' AS detail
  UNION ALL
  SELECT
    'group_by_position:B-1:-' AS decider_row,
    'group_by_position' AS decider,
    'B-1' AS basket_id,
    CAST(NULL AS VARCHAR) AS detail
  UNION ALL
  SELECT
    'group_by_position:B-2:-' AS decider_row,
    'group_by_position' AS decider,
    'B-2' AS basket_id,
    CAST(NULL AS VARCHAR) AS detail
  UNION ALL
  SELECT
    'group_by_position:B-4:-' AS decider_row,
    'group_by_position' AS decider,
    'B-4' AS basket_id,
    CAST(NULL AS VARCHAR) AS detail
  UNION ALL
  SELECT
    'order_by_limit:B-1:BL-03' AS decider_row,
    'order_by_limit' AS decider,
    'B-1' AS basket_id,
    'BL-03' AS detail
  UNION ALL
  SELECT
    'qualify:B-1:BL-01' AS decider_row,
    'qualify' AS decider,
    'B-1' AS basket_id,
    'BL-01' AS detail
  UNION ALL
  SELECT
    'qualify:B-2:BL-04' AS decider_row,
    'qualify' AS decider,
    'B-2' AS basket_id,
    'BL-04' AS detail
  UNION ALL
  SELECT
    'qualify:B-3:BL-06' AS decider_row,
    'qualify' AS decider,
    'B-3' AS basket_id,
    'BL-06' AS detail
  UNION ALL
  SELECT
    'qualify:B-4:BL-07' AS decider_row,
    'qualify' AS decider,
    'B-4' AS basket_id,
    'BL-07' AS detail
  UNION ALL
  SELECT
    'rollup_position:-:1:~=8' AS decider_row,
    'rollup_position' AS decider,
    CAST(NULL AS VARCHAR) AS basket_id,
    '1:~=8' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-1:0:bath=2' AS decider_row,
    'rollup_position' AS decider,
    'B-1' AS basket_id,
    '0:bath=2' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-1:0:gift=1' AS decider_row,
    'rollup_position' AS decider,
    'B-1' AS basket_id,
    '0:gift=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-1:1:~=3' AS decider_row,
    'rollup_position' AS decider,
    'B-1' AS basket_id,
    '1:~=3' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-2:0:bath=1' AS decider_row,
    'rollup_position' AS decider,
    'B-2' AS basket_id,
    '0:bath=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-2:0:garden=1' AS decider_row,
    'rollup_position' AS decider,
    'B-2' AS basket_id,
    '0:garden=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-2:1:~=2' AS decider_row,
    'rollup_position' AS decider,
    'B-2' AS basket_id,
    '1:~=2' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-3:0:pond=1' AS decider_row,
    'rollup_position' AS decider,
    'B-3' AS basket_id,
    '0:pond=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-3:1:~=1' AS decider_row,
    'rollup_position' AS decider,
    'B-3' AS basket_id,
    '1:~=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-4:0:gift=1' AS decider_row,
    'rollup_position' AS decider,
    'B-4' AS basket_id,
    '0:gift=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-4:0:~=1' AS decider_row,
    'rollup_position' AS decider,
    'B-4' AS basket_id,
    '0:~=1' AS detail
  UNION ALL
  SELECT
    'rollup_position:B-4:1:~=2' AS decider_row,
    'rollup_position' AS decider,
    'B-4' AS basket_id,
    '1:~=2' AS detail
  UNION ALL
  SELECT
    'where:B-1:BL-01' AS decider_row,
    'where' AS decider,
    'B-1' AS basket_id,
    'BL-01' AS detail
  UNION ALL
  SELECT
    'where:B-1:BL-03' AS decider_row,
    'where' AS decider,
    'B-1' AS basket_id,
    'BL-03' AS detail
  UNION ALL
  SELECT
    'where:B-2:BL-05' AS decider_row,
    'where' AS decider,
    'B-2' AS basket_id,
    'BL-05' AS detail
)
