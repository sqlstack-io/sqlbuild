MODEL (
  description """
    One row per partner with how many of its payload messages carry a colour, counted on the JSON
    path, on JSON nulls and on the colour read as text
  """,
  tags [edge_case],
  columns (
    partner_code (type VARCHAR, nullable false, audits [not_null, unique]),
    messages (type INT),
    colour_values (type INT),
    colour_json_nulls (type INT),
    colour_texts (type INT),
  ),
);

WITH payloads AS (
  SELECT
    s.partner_code,
    s.payload
  FROM __ref("stg_edge_partner_payloads") AS s
),

final AS (
  SELECT
    COALESCE(p.partner_code, 'UNKNOWN')::VARCHAR AS partner_code,
    COUNT(*)::INT AS messages,
    COUNT(p.payload:colour)::INT AS colour_values,
    COUNT_IF(IS_NULL_VALUE(p.payload:colour))::INT AS colour_json_nulls,
    COUNT(p.payload:colour::VARCHAR)::INT AS colour_texts
  FROM payloads AS p
  GROUP BY
    1
)

SELECT *
FROM final
