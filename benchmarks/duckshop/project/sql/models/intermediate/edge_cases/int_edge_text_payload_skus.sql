MODEL (
  description """
    One row per SKU named in partner message text, grouping and ranking by casts of fields parsed
    from the text inside the grouping and window keys themselves
  """,
  tags [edge_case],
  columns (
    sku (type VARCHAR),
    messages (type INT, audits [not_null]),
    carts (type INT),
    latest_message_id (type VARCHAR),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.received_at,
    s.payload_text
  FROM __ref("stg_edge_partner_payloads") AS s
),

parsed AS (
  SELECT
    p.message_id,
    p.received_at,
    TRY_PARSE_JSON(p.payload_text) AS document
  FROM payloads AS p
),

ranked AS (
  SELECT
    d.message_id,
    d.document,
    ROW_NUMBER() OVER (
      PARTITION BY d.document:sku::VARCHAR
      ORDER BY d.received_at DESC, d.message_id DESC
    ) AS recency
  FROM parsed AS d
),

final AS (
  SELECT
    r.document:sku::VARCHAR AS sku,
    COUNT(*)::INT AS messages,
    COUNT(DISTINCT r.document:cart_id::VARCHAR)::INT AS carts,
    MAX(IFF(r.recency = 1, r.message_id, NULL))::VARCHAR AS latest_message_id
  FROM ranked AS r
  GROUP BY
    r.document:sku::VARCHAR
)

SELECT *
FROM final
