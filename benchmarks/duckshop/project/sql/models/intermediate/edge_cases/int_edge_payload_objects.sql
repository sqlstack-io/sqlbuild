MODEL (
  description """
    One row per partner payload message with order summaries built as objects that drop or keep
    nulls, and the payload normalised to the current version by inserting, deleting and picking keys
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_summary (type OBJECT),
    order_summary_keep_null (type OBJECT),
    currency_object (type OBJECT),
    normalised_payload (type OBJECT),
    gift_note_updated (type OBJECT),
    picked_payload (type OBJECT),
    picked_by_array (type OBJECT),
    picked_by_variant (type OBJECT),
    payload_keys (type ARRAY),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.payload
  FROM __ref("stg_edge_partner_payloads") AS s
),

normalised AS (
  SELECT
    p.message_id,
    p.payload,
    OBJECT_INSERT(
      OBJECT_DELETE(
        p.payload::OBJECT,
        'meta',
        'a.b'
      ),
      'orderId',
      p.payload:meta:order_id,
      TRUE
    )::OBJECT AS normalised_payload
  FROM payloads AS p
),

final AS (
  SELECT
    n.message_id,
    OBJECT_CONSTRUCT(
      'channel',
      n.payload:channel::VARCHAR,
      'sku',
      n.payload:sku::VARCHAR,
      'qty',
      n.payload:qty
    )::OBJECT AS order_summary,
    OBJECT_CONSTRUCT_KEEP_NULL(
      'channel',
      n.payload:channel::VARCHAR,
      'sku',
      n.payload:sku::VARCHAR,
      'qty',
      n.payload:qty
    )::OBJECT AS order_summary_keep_null,
    {'currency': n.payload:currency::VARCHAR}::OBJECT AS currency_object,
    n.normalised_payload,
    OBJECT_INSERT(
      n.normalised_payload,
      'gift_note',
      n.payload:gift_note::VARCHAR,
      TRUE
    )::OBJECT AS gift_note_updated,
    OBJECT_PICK(n.normalised_payload, 'orderId', 'lines')::OBJECT AS picked_payload,
    OBJECT_PICK(
      n.payload::OBJECT,
      n.payload:pick_keys::ARRAY
    )::OBJECT AS picked_by_array,
    OBJECT_PICK(n.payload::OBJECT, n.payload:pick_keys)::OBJECT AS picked_by_variant,
    -- OBJECT_KEYS leaves key order unspecified; sort ascending.
    ARRAY_SORT(OBJECT_KEYS(n.payload::OBJECT), TRUE, FALSE)::ARRAY AS payload_keys
  FROM normalised AS n
)

SELECT *
FROM final
