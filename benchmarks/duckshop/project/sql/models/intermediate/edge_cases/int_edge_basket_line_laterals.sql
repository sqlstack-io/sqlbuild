MODEL (
  description """
    One row per line of every basket, read through the edge-case table function after LATERAL
  """,
  tags [edge_case],
  columns (
    basket_id (type VARCHAR),
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR),
  ),
);

WITH baskets AS (
  SELECT DISTINCT basket_id
  FROM __ref("stg_edge_basket_lines")
),

final AS (
  SELECT
    b.basket_id,
    l.line_id,
    l.sku
  FROM baskets AS b, LATERAL __table_fn("edge_basket_lines_for")(b.basket_id) AS l
)

SELECT *
FROM final
