MODEL (
  description """
    One row per pair of partner attribute values in a group, compared with = and EQUAL_NULL: 1
    against "1" and 1.0, JSON null against SQL NULL, objects with reordered keys, and a DATE or
    TIMESTAMP held in a VARIANT against the same text, which Snowflake keeps typed. Each value also
    carries its group's MIN_BY and MAX_BY VARIANT picks, how many rows share its value in a window
    partition, the group's distinct values by SELECT DISTINCT (counted with COUNT, which skips JSON
    null) and ARRAY_AGG(DISTINCT), a scalar cast of the value, and an OBJECT_CONSTRUCT star
    excluding a quoted lower-case column. Picks and objects that can hold typed values are output as
    their kind and JSON text, since JSON text alone would lose the type
  """,
  tags [edge_case],
  columns (
    pair_id (type VARCHAR, nullable false, audits [not_null, unique]),
    left_value (type VARIANT),
    right_value (type VARIANT),
    values_equal (type BOOLEAN),
    values_equal_null (type BOOLEAN),
    left_partition_size (type NUMBER(38, 0)),
    group_first_value (type VARCHAR),
    group_last_value (type VARCHAR),
    group_distinct_rows (type NUMBER(38, 0)),
    group_distinct_elements (type NUMBER(38, 0)),
    left_scalar_text (type VARCHAR),
    left_object_without_note (type VARCHAR),
  ),
);

WITH loaded AS (
  SELECT
    v.value_id,
    v.group_code,
    v.sort_key,
    v.attribute_value,
    v.value_text
  FROM __ref("stg_edge_variant_values") AS v
),

typed AS (
  -- A DATE and a TIMESTAMP stay typed inside a VARIANT, unlike the text they are written as.
  SELECT
    l.value_id || '-DATE' AS value_id,
    l.group_code,
    l.sort_key + 100 AS sort_key,
    TO_VARIANT(TRY_TO_DATE(l.attribute_value::VARCHAR)) AS attribute_value,
    l.value_text
  FROM loaded AS l
  WHERE
    IS_VARCHAR(l.attribute_value)
    AND TRY_TO_DATE(l.attribute_value::VARCHAR) IS NOT NULL
  UNION ALL
  SELECT
    l.value_id || '-TIMESTAMP' AS value_id,
    l.group_code,
    l.sort_key + 200 AS sort_key,
    TO_VARIANT(TRY_TO_TIMESTAMP_NTZ(l.attribute_value::VARCHAR)) AS attribute_value,
    l.value_text
  FROM loaded AS l
  WHERE
    IS_VARCHAR(l.attribute_value)
    AND TRY_TO_DATE(l.attribute_value::VARCHAR) IS NOT NULL
),

all_values AS (
  SELECT *
  FROM loaded
  UNION ALL BY NAME
  SELECT *
  FROM typed
),

with_note AS (
  SELECT
    v.value_id,
    v.group_code,
    v.sort_key,
    v.attribute_value,
    v.value_text AS "note"
  FROM all_values AS v
),

partitioned AS (
  SELECT
    a.value_id,
    a.group_code,
    a.sort_key,
    a.attribute_value,
    COUNT(*) OVER (PARTITION BY a.group_code, a.attribute_value) AS partition_size,
    OBJECT_CONSTRUCT(* EXCLUDE ("note")) AS object_without_note
  FROM with_note AS a
),

group_picks AS (
  SELECT
    a.group_code,
    MIN_BY(a.attribute_value, a.sort_key) AS group_first_value,
    MAX_BY(a.attribute_value, a.sort_key) AS group_last_value,
    ARRAY_SIZE(ARRAY_AGG(DISTINCT a.attribute_value)) AS group_distinct_elements
  FROM all_values AS a
  GROUP BY
    a.group_code
),

distinct_values AS (
  SELECT DISTINCT
    a.group_code,
    a.attribute_value
  FROM all_values AS a
),

distinct_counts AS (
  SELECT
    d.group_code,
    COUNT(d.attribute_value) AS group_distinct_rows
  FROM distinct_values AS d
  GROUP BY
    d.group_code
),

final AS (
  SELECT
    (
      l.value_id || '~' || r.value_id
    )::VARCHAR AS pair_id,
    l.attribute_value AS left_value,
    r.attribute_value AS right_value,
    (
      l.attribute_value = r.attribute_value
    )::BOOLEAN AS values_equal,
    EQUAL_NULL(l.attribute_value, r.attribute_value)::BOOLEAN AS values_equal_null,
    l.partition_size::NUMBER(38, 0) AS left_partition_size,
    (
      TYPEOF(g.group_first_value) || ' ' || TO_JSON(g.group_first_value)
    )::VARCHAR AS group_first_value,
    (
      TYPEOF(g.group_last_value) || ' ' || TO_JSON(g.group_last_value)
    )::VARCHAR AS group_last_value,
    c.group_distinct_rows::NUMBER(38, 0) AS group_distinct_rows,
    g.group_distinct_elements::NUMBER(38, 0) AS group_distinct_elements,
    IFF(
      IS_OBJECT(l.attribute_value) OR IS_ARRAY(l.attribute_value),
      NULL,
      l.attribute_value
    )::VARCHAR AS left_scalar_text,
    TO_JSON(l.object_without_note)::VARCHAR AS left_object_without_note
  FROM partitioned AS l
  INNER JOIN partitioned AS r
    ON l.group_code = r.group_code
    AND l.sort_key < r.sort_key
  INNER JOIN group_picks AS g
    ON l.group_code = g.group_code
  INNER JOIN distinct_counts AS c
    ON l.group_code = c.group_code
)

SELECT *
FROM final
