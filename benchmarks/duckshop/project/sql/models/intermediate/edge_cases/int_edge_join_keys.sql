MODEL (
  description """
    One row per check group matched to a group name by join shapes whose merged keys and outer sides
    are easy to trace wrongly: RIGHT JOIN ... USING (the key comes from the right), NATURAL JOIN, a
    USING key followed by a later RIGHT JOIN that makes it NULL, and an ASOF JOIN whose unmatched
    rows hold NULL on the right
  """,
  tags [edge_case],
  columns (
    row_key (type VARCHAR, nullable false, audits [not_null, unique]),
    join_shape (type VARCHAR),
    group_code (type VARCHAR),
    group_name (type VARCHAR),
    threshold_name (type VARCHAR),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.group_code,
    c.narrow_key
  FROM __ref("stg_edge_amount_checks") AS c
),

check_groups AS (
  SELECT DISTINCT c.group_code
  FROM checks AS c
  WHERE c.group_code <> 'G-3'
),

group_names AS (
  SELECT
    v.group_code,
    v.group_name
  FROM VALUES
    ('G-1', 'Mallard desk'),
    ('G-2', 'Teal desk'),
    ('G-9', 'Eider desk') AS v(group_code, group_name)
),

desks AS (
  SELECT v.desk_code
  FROM VALUES ('G-1'), ('G-7') AS v(desk_code)
),

thresholds AS (
  SELECT
    v.threshold_key,
    v.threshold_name
  FROM VALUES (10, 'ten or more'), (100, 'a hundred or more') AS v(threshold_key, threshold_name)
),

right_using AS (
  SELECT
    'right_using' AS join_shape,
    group_code,
    n.group_name,
    NULL AS threshold_name
  FROM check_groups AS g
  RIGHT JOIN group_names AS n
    USING (group_code)
),

natural_joined AS (
  SELECT
    'natural' AS join_shape,
    n.group_code,
    n.group_name,
    NULL AS threshold_name
  FROM check_groups AS g
  NATURAL JOIN group_names AS n
),

later_outer AS (
  SELECT
    'using_then_right' AS join_shape,
    group_code,
    n.group_name,
    d.desk_code AS threshold_name
  FROM check_groups AS g
  INNER JOIN group_names AS n
    USING (group_code)
  RIGHT JOIN desks AS d
    ON n.group_code = d.desk_code
),

asof_matched AS (
  SELECT
    'asof:' || c.check_id AS join_shape,
    c.group_code,
    NULL AS group_name,
    t.threshold_name
  FROM checks AS c
  ASOF JOIN thresholds AS t MATCH_CONDITION (c.narrow_key >= t.threshold_key)
),

all_rows AS (
  SELECT *
  FROM right_using
  UNION ALL BY NAME
  SELECT *
  FROM natural_joined
  UNION ALL BY NAME
  SELECT *
  FROM later_outer
  UNION ALL BY NAME
  SELECT *
  FROM asof_matched
),

final AS (
  SELECT
    (
      a.join_shape || ':' || COALESCE(a.group_code, '-') || ':' || COALESCE(a.threshold_name, '-')
    )::VARCHAR AS row_key,
    a.join_shape::VARCHAR AS join_shape,
    a.group_code::VARCHAR AS group_code,
    a.group_name::VARCHAR AS group_name,
    a.threshold_name::VARCHAR AS threshold_name
  FROM all_rows AS a
)

SELECT *
FROM final
