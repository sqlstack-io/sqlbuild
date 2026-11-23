MODEL (
  description """
    One row per basket line with the first two parts of its SKU, read through a comma-joined LATERAL
    whose nested derived table reads the line
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku_prefix (type VARCHAR),
    sku_colour (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    l.line_id,
    x.sku_prefix::VARCHAR AS sku_prefix,
    x.sku_colour::VARCHAR AS sku_colour
  FROM __ref("stg_edge_basket_lines") AS l, LATERAL (
    SELECT
      p.sku_prefix,
      p.sku_colour
    FROM (
      SELECT
        SPLIT_PART(l.sku, '-', 1) AS sku_prefix,
        SPLIT_PART(l.sku, '-', 2) AS sku_colour
    ) AS p
  ) AS x
)

SELECT *
FROM final
