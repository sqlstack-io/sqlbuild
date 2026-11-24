MODEL (
  description """
    One row per partner payload message and attribute name the partner asked for, with the attribute
    read by a computed key, exactly and ignoring case
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null]),
    wanted_index (type INT, nullable false, audits [not_null]),
    attribute_name (type VARCHAR),
    attribute_value (type VARCHAR),
    attribute_value_any_case (type VARCHAR),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.payload
  FROM __ref("stg_edge_partner_payloads") AS s
),

final AS (
  SELECT
    p.message_id,
    w.index::INT AS wanted_index,
    w.value::VARCHAR AS attribute_name,
    GET(p.payload:attrs, w.value::VARCHAR)::VARCHAR AS attribute_value,
    GET_IGNORE_CASE(p.payload:attrs, w.value::VARCHAR)::VARCHAR AS attribute_value_any_case
  FROM payloads AS p, LATERAL FLATTEN(INPUT => p.payload:wanted) AS w
)

SELECT *
FROM final
