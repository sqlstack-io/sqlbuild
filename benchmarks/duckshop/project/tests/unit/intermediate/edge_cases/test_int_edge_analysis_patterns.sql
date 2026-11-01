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
    NULL AS basket_id,
    NULL AS month_end,
    NULL AS line_no,
    NULL AS positional_sku,
    NULL AS next_line_no,
    NULL AS line_no_text,
    NULL AS sku_prefix,
    NULL AS line_count
)
