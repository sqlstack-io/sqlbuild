MODEL (
  description """
    One row per attribute value holding an XML document, with its sku element read by XMLGET
  """,
  tags [edge_case],
  columns (
    value_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku_text (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    v.value_id,
    XMLGET(v.attribute_value, 'sku'):"$"::VARCHAR AS sku_text
  FROM __ref("stg_edge_variant_values") AS v
)

SELECT *
FROM final
