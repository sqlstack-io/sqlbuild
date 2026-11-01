TEST (name "stg_edge_basket_lines__lower_cases_and_trims_tags");

WITH __source__edge__basket_lines AS (
  SELECT
    v.line_id,
    v.basket_id,
    v.line_no::NUMBER(38, 0) AS line_no,
    v.sku,
    v.tag,
    '2026-03-06 00:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('BL-01', 'B-1', 1, 'MALLARD-01', ' Bath '),
    ('BL-02', 'B-1', 2, 'TEAL-02', 'GIFT'),
    ('BL-03', 'B-2', 1, 'EIDER-03', NULL) AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__stg_edge_basket_lines AS (
  SELECT
    NULL AS line_id,
    NULL AS basket_id,
    NULL AS line_no,
    NULL AS tag
)
