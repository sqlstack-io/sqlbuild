MODEL (
  description """
    One row per tag in the comma-separated tag text a partner sends, including empty tags between
    commas
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null]),
    tag_index (type INT),
    tag (type VARCHAR),
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
    t.index::INT AS tag_index,
    t.value::VARCHAR AS tag
  FROM payloads AS p, LATERAL SPLIT_TO_TABLE(
    p.payload:tags_text::VARCHAR,
    ','
  ) AS t
)

SELECT *
FROM final
