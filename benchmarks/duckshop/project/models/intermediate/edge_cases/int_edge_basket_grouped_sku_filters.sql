MODEL (
  description """
    One row per basket with the number of its red lines and of its SKUs longer than nine characters,
    counted by an untyped and a typed FILTER lambda over ARRAY_AGG in the grouped query
  """,
  tags [edge_case],
  columns (
    basket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    red_line_count (type NUMBER(38, 0)),
    long_sku_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    l.basket_id,
    ARRAY_SIZE(
      FILTER(
        ARRAY_AGG(l.sku) WITHIN GROUP (ORDER BY l.line_no, l.line_id),
        s -> s::VARCHAR LIKE 'DK-RED-%'
      )
    )::NUMBER(38, 0) AS red_line_count,
    ARRAY_SIZE(
      FILTER(
        ARRAY_AGG(l.sku) WITHIN GROUP (ORDER BY l.line_no, l.line_id),
        s VARCHAR -> LENGTH(s) > 9
      )
    )::NUMBER(38, 0) AS long_sku_count
  FROM __ref("stg_edge_basket_lines") AS l
  GROUP BY
    l.basket_id
)

SELECT *
FROM final
