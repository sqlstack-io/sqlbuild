MODEL (
  description """
    One row per partner payload message, published with an event document built from typed values: a
    nullable channel, a FLOAT unit price, an exact amount and the placement date
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR),
    event_document (type VARIANT),
    event_document_text (type VARCHAR),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.partner_code,
    s.payload,
    s.amount_float,
    s.amount,
    s.placed_on
  FROM __ref("stg_edge_partner_payloads") AS s
),

documents AS (
  SELECT
    p.message_id,
    p.partner_code,
    OBJECT_CONSTRUCT_KEEP_NULL(
      'channel',
      p.payload:channel::VARCHAR,
      'unit_price',
      p.amount_float,
      'amount',
      p.amount,
      'placed_on',
      p.placed_on
    )::VARIANT AS event_document
  FROM payloads AS p
),

final AS (
  SELECT
    d.message_id,
    d.partner_code,
    d.event_document,
    d.event_document::VARCHAR AS event_document_text
  FROM documents AS d
)

SELECT *
FROM final
