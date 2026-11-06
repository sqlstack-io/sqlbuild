MODEL (
  description """
    One row per element of a partner attribute document flattened with RECURSIVE, with the PATH
    Snowflake writes for keys holding $, -, @, [, a space, a non-ASCII letter or nothing unusual,
    and for array indexes inside them
  """,
  tags [edge_case],
  columns (
    element_id (type VARCHAR, nullable false, audits [not_null, unique]),
    value_id (type VARCHAR),
    element_path (type VARCHAR),
    element_key (type VARCHAR),
    element_index (type NUMBER(38, 0)),
    element_kind (type VARCHAR),
  ),
);

WITH documents AS (
  SELECT
    v.value_id,
    v.attribute_value
  FROM __ref("stg_edge_variant_values") AS v
  WHERE
    IS_OBJECT(v.attribute_value)
    OR IS_ARRAY(v.attribute_value)
),

elements AS (
  SELECT
    d.value_id,
    f.path,
    f.key,
    f.index,
    f.value
  FROM documents AS d, LATERAL FLATTEN(input => d.attribute_value, recursive => TRUE) AS f
),

final AS (
  SELECT
    (
      e.value_id || ':' || e.path
    )::VARCHAR AS element_id,
    e.value_id,
    e.path::VARCHAR AS element_path,
    e.key::VARCHAR AS element_key,
    e.index::NUMBER(38, 0) AS element_index,
    TYPEOF(e.value)::VARCHAR AS element_kind
  FROM elements AS e
)

SELECT *
FROM final
