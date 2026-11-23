MODEL (
  description """
    One row per published payload event, reading the typed values back out of the event document
    built in another model: the kind Snowflake kept for each value and the values cast again
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    unit_price_kind (type VARCHAR),
    unit_price (type FLOAT),
    amount_kind (type VARCHAR),
    amount (type NUMBER(18, 2)),
    placed_on_kind (type VARCHAR),
    placed_on (type DATE),
    placed_on_text (type VARCHAR),
    channel_kind (type VARCHAR),
  ),
);

WITH events AS (
  SELECT
    e.message_id,
    e.event_document
  FROM __ref("fct_edge_payload_events") AS e
),

final AS (
  SELECT
    e.message_id,
    TYPEOF(e.event_document:unit_price)::VARCHAR AS unit_price_kind,
    e.event_document:unit_price::FLOAT AS unit_price,
    TYPEOF(e.event_document:amount)::VARCHAR AS amount_kind,
    e.event_document:amount::NUMBER(18, 2) AS amount,
    TYPEOF(e.event_document:placed_on)::VARCHAR AS placed_on_kind,
    e.event_document:placed_on::DATE AS placed_on,
    e.event_document:placed_on::VARCHAR AS placed_on_text,
    TYPEOF(e.event_document:channel)::VARCHAR AS channel_kind
  FROM events AS e
)

SELECT *
FROM final
