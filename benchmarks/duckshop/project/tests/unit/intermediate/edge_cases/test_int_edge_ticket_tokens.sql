TEST (name "int_edge_ticket_tokens__calls_volatile_functions_once_per_row");

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

__expected__int_edge_ticket_tokens AS (
  SELECT
    NULL AS line_id,
    NULL AS edge_next_ticket_source,
    NULL AS look_alike_text,
    NULL AS tag_label
),

__assert__every_call_gives_a_new_value AS (
  SELECT COUNT(*) AS line_count
  FROM __ref("int_edge_ticket_tokens") AS t
  HAVING
    COUNT(DISTINCT t.line_token) <> COUNT(*)
    OR COUNT(DISTINCT t.constant_seed_token) <> COUNT(*)
    OR COUNT(DISTINCT t.next_ticket) <> COUNT(*)
),

__assert__half_tickets_are_positive AS (
  SELECT t.line_id
  FROM __ref("int_edge_ticket_tokens") AS t
  WHERE
    t.half_ticket IS NULL
    OR t.half_ticket <= 0
)
