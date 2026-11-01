TEST (name "int_edge_analysis_patterns__reads_positional_and_unaliased_columns");

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

__ref__stg_edge_date_texts AS (
  SELECT
    v.text_id,
    v.date_text,
    NULL AS zoned_text,
    NULL::TIMESTAMP_NTZ(3) AS wall_clock,
    NULL AS wall_clock_text,
    NULL::TIME(3) AS local_time,
    NULL::TIME(9) AS fine_time
  FROM VALUES
    ('DT-01', '2026-03-05'),
    ('DT-02', '2026-02-14'),
    ('DT-03', 'not a date') AS v(text_id, date_text)
),

__expected__int_edge_analysis_patterns AS (
  SELECT
    'B-1' AS basket_id,
    '2026-02-28'::DATE AS month_end,
    3 AS line_no,
    'EIDER-03' AS positional_sku,
    4 AS next_line_no,
    '1' AS line_no_text,
    'EIDER' AS sku_prefix,
    3 AS line_count
  UNION ALL
  SELECT
    'B-2' AS basket_id,
    '2026-02-28'::DATE AS month_end,
    2 AS line_no,
    'MALLARD-01' AS positional_sku,
    3 AS next_line_no,
    '1' AS line_no_text,
    'MALLA' AS sku_prefix,
    2 AS line_count
  UNION ALL
  SELECT
    'B-3' AS basket_id,
    '2026-02-28'::DATE AS month_end,
    1 AS line_no,
    'MERGANSER-05' AS positional_sku,
    2 AS next_line_no,
    '1' AS line_no_text,
    'MERGA' AS sku_prefix,
    1 AS line_count
  UNION ALL
  SELECT
    'B-4' AS basket_id,
    '2026-02-28'::DATE AS month_end,
    2 AS line_no,
    'EIDER-03' AS positional_sku,
    3 AS next_line_no,
    '1' AS line_no_text,
    'EIDER' AS sku_prefix,
    2 AS line_count
)
