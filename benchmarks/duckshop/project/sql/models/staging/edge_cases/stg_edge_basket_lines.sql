MODEL (
  description """
    One row per basket line with its tag in lower case, the input to the edge models that collect
    tags into arrays whose element order Snowflake does not define
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    basket_id (type VARCHAR),
    line_no (type NUMBER(38, 0)),
    sku (type VARCHAR),
    tag (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__basket_lines")
),

final AS (
  SELECT
    s.line_id,
    s.basket_id,
    s.line_no,
    s.sku,
    LOWER(TRIM(s.tag))::VARCHAR AS tag
  FROM src AS s
)

SELECT *
FROM final
