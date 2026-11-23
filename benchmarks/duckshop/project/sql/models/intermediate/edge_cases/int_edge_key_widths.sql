MODEL (
  description """
    One row per check pair matched by JOIN ... USING between a NUMBER(18,0) key and a NUMBER(38,0)
    key, inner, left and full, including a 23-digit key, with the types Snowflake gives the merged
    key and each side's qualified key; and a text key joined to a number key, where Snowflake
    compares the text as a number
  """,
  tags [edge_case],
  columns (
    pair_id (type VARCHAR, nullable false, audits [not_null, unique]),
    join_kind (type VARCHAR),
    check_key_text (type VARCHAR),
    narrow_check_id (type VARCHAR),
    wide_check_id (type VARCHAR),
    merged_key_type (type VARCHAR),
    narrow_side_type (type VARCHAR),
    wide_side_type (type VARCHAR),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.narrow_key,
    c.wide_key,
    c.key_text
  FROM __ref("stg_edge_amount_checks") AS c
),

narrow AS (
  SELECT
    c.check_id AS narrow_check_id,
    c.narrow_key AS check_key
  FROM checks AS c
  WHERE c.narrow_key IS NOT NULL
),

wide AS (
  SELECT
    c.check_id AS wide_check_id,
    c.wide_key AS check_key
  FROM checks AS c
  WHERE c.wide_key IS NOT NULL
),

text_keys AS (
  SELECT
    c.check_id AS text_check_id,
    c.key_text AS check_key
  FROM checks AS c
  WHERE TRY_TO_NUMBER(c.key_text) IS NOT NULL
),

inner_pairs AS (
  SELECT
    'inner' AS join_kind,
    check_key::VARCHAR AS check_key_text,
    n.narrow_check_id,
    w.wide_check_id,
    SYSTEM$TYPEOF(check_key) AS merged_key_type,
    SYSTEM$TYPEOF(n.check_key) AS narrow_side_type,
    SYSTEM$TYPEOF(w.check_key) AS wide_side_type
  FROM narrow AS n
  INNER JOIN wide AS w
    USING (check_key)
),

left_pairs AS (
  SELECT
    'left' AS join_kind,
    check_key::VARCHAR AS check_key_text,
    n.narrow_check_id,
    w.wide_check_id,
    SYSTEM$TYPEOF(check_key) AS merged_key_type,
    SYSTEM$TYPEOF(n.check_key) AS narrow_side_type,
    SYSTEM$TYPEOF(w.check_key) AS wide_side_type
  FROM narrow AS n
  LEFT JOIN wide AS w
    USING (check_key)
),

full_pairs AS (
  SELECT
    'full' AS join_kind,
    check_key::VARCHAR AS check_key_text,
    n.narrow_check_id,
    w.wide_check_id,
    SYSTEM$TYPEOF(check_key) AS merged_key_type,
    SYSTEM$TYPEOF(n.check_key) AS narrow_side_type,
    SYSTEM$TYPEOF(w.check_key) AS wide_side_type
  FROM narrow AS n
  FULL JOIN wide AS w
    USING (check_key)
),

text_pairs AS (
  SELECT
    'text' AS join_kind,
    check_key::VARCHAR AS check_key_text,
    t.text_check_id AS narrow_check_id,
    w.wide_check_id,
    SYSTEM$TYPEOF(check_key) AS merged_key_type,
    SYSTEM$TYPEOF(t.check_key) AS narrow_side_type,
    SYSTEM$TYPEOF(w.check_key) AS wide_side_type
  FROM text_keys AS t
  INNER JOIN wide AS w
    USING (check_key)
),

all_pairs AS (
  SELECT *
  FROM inner_pairs
  UNION ALL BY NAME
  SELECT *
  FROM left_pairs
  UNION ALL BY NAME
  SELECT *
  FROM full_pairs
  UNION ALL BY NAME
  SELECT *
  FROM text_pairs
),

final AS (
  SELECT
    (
      a.join_kind
        || ':'
        || COALESCE(a.narrow_check_id, '-')
        || ':'
        || COALESCE(a.wide_check_id, '-')
    )::VARCHAR AS pair_id,
    a.join_kind::VARCHAR AS join_kind,
    a.check_key_text::VARCHAR AS check_key_text,
    a.narrow_check_id::VARCHAR AS narrow_check_id,
    a.wide_check_id::VARCHAR AS wide_check_id,
    REGEXP_REPLACE(a.merged_key_type, '\\[.*\\]$', '')::VARCHAR AS merged_key_type,
    REGEXP_REPLACE(a.narrow_side_type, '\\[.*\\]$', '')::VARCHAR AS narrow_side_type,
    REGEXP_REPLACE(a.wide_side_type, '\\[.*\\]$', '')::VARCHAR AS wide_side_type
  FROM all_pairs AS a
)

SELECT *
FROM final
