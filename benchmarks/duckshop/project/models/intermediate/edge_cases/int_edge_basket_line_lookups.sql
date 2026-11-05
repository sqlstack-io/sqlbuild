MODEL (
  description """
    One row per line of one fixed basket, read through the edge-case table function
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    line_no (type INT),
    sku (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    l.line_id,
    l.line_no,
    l.sku
  FROM __table_fn("edge_basket_lines_for")('B-1') AS l
)

SELECT *
FROM final
