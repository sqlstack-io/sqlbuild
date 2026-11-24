MODEL (
  description """
    One row per amount check with its group name read through relations aliased first and last, the
    words of Snowflake's read-order clauses: each alias names its relation, so neither join is
    unused, and a condition on the last relation decides which of its rows match
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    first_group_name (type VARCHAR),
    last_group_name (type VARCHAR),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.group_code
  FROM __ref("stg_edge_amount_checks") AS c
),

group_names AS (
  SELECT
    v.group_code,
    v.group_name
  FROM VALUES ('G-1', 'Mallard desk'), ('G-2', 'Teal desk') AS v(group_code, group_name)
),

final AS (
  SELECT
    c.check_id::VARCHAR AS check_id,
    first.group_name::VARCHAR AS first_group_name,
    last.group_name::VARCHAR AS last_group_name
  FROM checks AS c
  LEFT JOIN group_names AS first ON c.group_code = first.group_code
  LEFT JOIN group_names last
    ON c.group_code = last.group_code
    AND last.group_name <> 'Teal desk'
)

SELECT *
FROM final
