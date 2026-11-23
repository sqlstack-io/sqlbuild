MODEL (
  description """
    One row per basket line with the SKUs of its neighbouring lines in the basket, read by LAG and
    LEAD with one, two and three arguments
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    previous_sku (type VARCHAR),
    second_previous_sku (type VARCHAR),
    next_sku (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    l.line_id,
    LAG(l.sku) OVER (
      PARTITION BY l.basket_id
      ORDER BY l.line_no, l.line_id
    )::VARCHAR AS previous_sku,
    LAG(l.sku, 2) OVER (
      PARTITION BY l.basket_id
      ORDER BY l.line_no, l.line_id
    )::VARCHAR AS second_previous_sku,
    LEAD(l.sku, 1, 'NONE') OVER (
      PARTITION BY l.basket_id
      ORDER BY l.line_no, l.line_id
    )::VARCHAR AS next_sku
  FROM __ref("stg_edge_basket_lines") AS l
)

SELECT *
FROM final
