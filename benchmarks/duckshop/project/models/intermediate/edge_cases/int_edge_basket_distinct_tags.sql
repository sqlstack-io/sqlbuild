MODEL (
  description """
    One row per distinct tag of the basket lines, held as VARIANT and read back as text
  """,
  tags [edge_case],
  columns (
    tag_text (type VARCHAR, nullable false, audits [not_null, unique]),
  ),
);

WITH distinct_tags AS (
  SELECT DISTINCT TO_VARIANT(l.tag) AS tag_value
  FROM __ref("stg_edge_basket_lines") AS l
),

final AS (
  SELECT d.tag_value::VARCHAR AS tag_text
  FROM distinct_tags AS d
  WHERE d.tag_value IS NOT NULL
)

SELECT *
FROM final
