MODEL (
  description """
    One row per attribute value with its text stripped of separators by a pattern from the group
    code, and CHECK_XML of the text
  """,
  tags [edge_case],
  columns (
    value_id (type VARCHAR, nullable false, audits [not_null, unique]),
    stripped_text (type VARCHAR),
    xml_error (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    v.value_id,
    REGEXP_REPLACE(v.value_text, v.group_code, '') AS stripped_text,
    CHECK_XML(v.value_text)::VARCHAR AS xml_error
  FROM __ref("stg_edge_variant_values") AS v
)

SELECT *
FROM final
