MODEL (
  description """
    One row per supplier and quarter with purchase order fill, lateness and received damage
  """,
  columns (
    scorecard_key (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_code (type VARCHAR),
    quarter_start (type DATE),
    po_lines (type INT),
    ordered_qty (type INT),
    received_qty (type INT),
    fill_rate (type NUMBER(9, 4)),
    late_line_share (type NUMBER(9, 4)),
    damage_rate (type NUMBER(9, 4)),
    discrepancy_codes (type ARRAY),
  ),
);

WITH po_lines AS (
  SELECT *
  FROM __ref("int_purchase_order_lines_received")
),

quarterly AS (
  SELECT
    p.supplier_code,
    DATE_TRUNC('quarter', p.first_received_at_local)::DATE AS quarter_start,
    COUNT(*) AS po_lines,
    SUM(p.ordered_qty) AS ordered_qty,
    SUM(p.received_qty) AS received_qty,
    AVG(IFF(p.days_late > 0, 1, 0)) AS late_line_share,
    SUM(p.damaged_qty) AS damaged_qty,
    ARRAY_DISTINCT(ARRAY_FLATTEN(ARRAY_AGG(p.discrepancy_codes))) AS discrepancy_codes
  FROM po_lines AS p
  WHERE p.first_received_at_local IS NOT NULL
  GROUP BY
    1,
    2
),

final AS (
  SELECT
    (
      q.supplier_code || ':' || TO_VARCHAR(q.quarter_start, 'YYYY-MM-DD')
    )::VARCHAR AS scorecard_key,
    q.supplier_code,
    q.quarter_start,
    q.po_lines::INT AS po_lines,
    q.ordered_qty::INT AS ordered_qty,
    q.received_qty::INT AS received_qty,
    ROUND(q.received_qty / NULLIF(q.ordered_qty, 0), 4)::NUMBER(9, 4) AS fill_rate,
    ROUND(q.late_line_share, 4)::NUMBER(9, 4) AS late_line_share,
    ROUND(q.damaged_qty / NULLIF(q.received_qty, 0), 4)::NUMBER(9, 4) AS damage_rate,
    q.discrepancy_codes::ARRAY AS discrepancy_codes
  FROM quarterly AS q
)

SELECT *
FROM final
