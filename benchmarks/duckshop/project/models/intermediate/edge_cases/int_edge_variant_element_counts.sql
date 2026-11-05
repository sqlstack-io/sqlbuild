MODEL (
  description """
    One row per value group counting the elements of its attribute values, flattened without an
    alias
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    element_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    v.group_code,
    COUNT(*)::NUMBER(38, 0) AS element_count
  FROM __ref("stg_edge_variant_values") AS v,
    LATERAL FLATTEN(INPUT => v.attribute_value)
  GROUP BY
    v.group_code
)

SELECT *
FROM final
