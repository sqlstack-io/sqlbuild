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
    'BL-01' AS line_id,
    'BL-01' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Bath' AS tag_label
  UNION ALL
  SELECT
    'BL-02' AS line_id,
    'BL-02' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Gift' AS tag_label
  UNION ALL
  SELECT
    'BL-03' AS line_id,
    'BL-03' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Bath' AS tag_label
  UNION ALL
  SELECT
    'BL-04' AS line_id,
    'BL-04' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Garden' AS tag_label
  UNION ALL
  SELECT
    'BL-05' AS line_id,
    'BL-05' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Bath' AS tag_label
  UNION ALL
  SELECT
    'BL-06' AS line_id,
    'BL-06' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Pond' AS tag_label
  UNION ALL
  SELECT
    'BL-07' AS line_id,
    'BL-07' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Untagged' AS tag_label
  UNION ALL
  SELECT
    'BL-08' AS line_id,
    'BL-08' AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket' AS look_alike_text,
    'Gift' AS tag_label
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
