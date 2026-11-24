MODEL (
  description """
    One row per basket line with the number of parts of its SKU longer than one character, kept
    by FILTER
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    long_part_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    l.line_id,
    ARRAY_SIZE(FILTER(SPLIT(l.sku, '-'), part -> LENGTH(part) > 1))::NUMBER(38, 0) AS long_part_count
  FROM __ref("stg_edge_basket_lines") AS l
)

SELECT *
FROM final
