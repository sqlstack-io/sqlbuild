MODEL (
  description """
    One row per partner attribute value held as VARIANT, never SQL NULL (a missing value arrives as
    JSON null), with the raw text beside it
  """,
  tags [edge_case],
  columns (
    value_id (type VARCHAR, nullable false, audits [not_null, unique]),
    group_code (type VARCHAR),
    sort_key (type NUMBER(38, 0)),
    attribute_value (type VARIANT, nullable false, audits [not_null]),
    value_text (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__variant_values")
),

final AS (
  SELECT
    s.value_id,
    s.group_code,
    s.sort_key,
    COALESCE(s.attribute_value, PARSE_JSON('null')) AS attribute_value,
    s.value_text
  FROM src AS s
)

SELECT *
FROM final
