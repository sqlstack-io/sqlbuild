MODEL (
  description """
    One row per attribute value that differs from the value expected for its group, compared
    with EQUAL_NULL
  """,
  tags [edge_case],
  columns (
    value_id (type VARCHAR, nullable false, audits [not_null, unique]),
  ),
);

WITH final AS (
  SELECT v.value_id
  FROM __ref("stg_edge_variant_values") AS v
  WHERE NOT EQUAL_NULL(
    v.attribute_value,
    CASE v.group_code WHEN 'V-1' THEN PARSE_JSON('1.5') END
  )
)

SELECT *
FROM final
