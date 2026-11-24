MODEL (
  description """
    One row per partner payload line, keeping messages whose lines are empty, a JSON null, missing
    or a single object as one row without a line
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null]),
    line_index (type INT),
    line_key (type VARCHAR),
    line_path (type VARCHAR),
    line_kind (type VARCHAR),
    line_sku (type VARCHAR),
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
    l.index::INT AS line_index,
    l.key::VARCHAR AS line_key,
    l.path::VARCHAR AS line_path,
    TYPEOF(l.value)::VARCHAR AS line_kind,
    l.value:sku::VARCHAR AS line_sku
  FROM payloads AS p, LATERAL FLATTEN(INPUT => p.payload:lines, OUTER => TRUE) AS l
)

SELECT *
FROM final
