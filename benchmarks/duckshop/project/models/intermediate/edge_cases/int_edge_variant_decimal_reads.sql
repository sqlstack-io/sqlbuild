MODEL (
  description """
    One row per attribute value with the value read as a NUMBER(10, 4) by TO_DECIMAL
  """,
  tags [edge_case],
  columns (
    value_id (type VARCHAR, nullable false, audits [not_null, unique]),
    decimal_value (type NUMBER(10, 4)),
  ),
);

WITH final AS (
  SELECT
    v.value_id,
    TO_DECIMAL(v.attribute_value, 10, 4)::NUMBER(10, 4) AS decimal_value
  FROM __ref("stg_edge_variant_values") AS v
  WHERE IS_DECIMAL(v.attribute_value) OR IS_INTEGER(v.attribute_value)
)

SELECT *
FROM final
