MODEL (
  description """
    One row per check whose JSON number text is parsed with TRY_PARSE_JSON: whole numbers written
    with zero fractions, invalid numbers with two decimal points, a 38-digit number ending in .0, a
    one followed by forty zero decimals, negative zero and exponents, with the parsed kind and the
    JSON text Snowflake writes back
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    json_text (type VARCHAR),
    parsed_kind (type VARCHAR),
    parsed_text (type VARCHAR),
    amount_kind (type VARCHAR),
    amount_text (type VARCHAR),
    is_valid_json (type BOOLEAN),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.json_text
  FROM __ref("stg_edge_amount_checks") AS c
),

parsed AS (
  SELECT
    c.check_id,
    c.json_text,
    TRY_PARSE_JSON(c.json_text) AS document
  FROM checks AS c
),

final AS (
  SELECT
    p.check_id,
    p.json_text,
    TYPEOF(p.document)::VARCHAR AS parsed_kind,
    TO_JSON(p.document)::VARCHAR AS parsed_text,
    TYPEOF(p.document:amount)::VARCHAR AS amount_kind,
    p.document:amount::VARCHAR AS amount_text,
    CHECK_JSON(p.json_text) IS NULL AS is_valid_json
  FROM parsed AS p
)

SELECT *
FROM final
