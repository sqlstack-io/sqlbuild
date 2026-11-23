FUNCTION (
  description "Lines of one basket with their SKU and tag, for the edge-case table-function reader",
  tags [edge_case],
  arguments (p_basket_id VARCHAR),
  returns table (
    line_id VARCHAR,
    line_no INT,
    sku VARCHAR,
    tag VARCHAR
  )
);

WITH lines AS (
  SELECT *
  FROM __ref("stg_edge_basket_lines")
),

final AS (
  SELECT
    l.line_id,
    l.line_no,
    l.sku,
    l.tag
  FROM lines AS l
  WHERE l.basket_id = p_basket_id
)

SELECT *
FROM final
