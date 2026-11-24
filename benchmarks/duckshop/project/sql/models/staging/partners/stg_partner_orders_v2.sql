MODEL (
  description """
    One row per partner order received in payload version 2, where amounts are minor units and times
    are epoch milliseconds
  """,
  columns (
    partner_order_key (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    external_order_id (type VARCHAR, audits [not_null]),
    buyer_email_hash (type VARCHAR),
    currency_code (type VARCHAR),
    order_total_minor (type NUMBER(38, 0)),
    placed_at_utc (type TIMESTAMP_NTZ),
    items (type ARRAY),
  ),
);

WITH messages AS (
  SELECT *
  FROM __ref("stg_partner_orders")
),

final AS (
  SELECT
    (
      m.partner_code || ':' || m.external_order_id
    )::VARCHAR AS partner_order_key,
    m.partner_code,
    m.external_order_id,
    LOWER(m.payload:customer.email_hash::VARCHAR)::VARCHAR AS buyer_email_hash,
    UPPER(m.payload:order.currency::VARCHAR)::VARCHAR AS currency_code,
    m.payload:order.totals.gross_minor::NUMBER(38, 0) AS order_total_minor,
    @sql.epoch_ms_to_utc(m.payload:order.placed_at_ms::NUMBER(38, 0)) AS placed_at_utc,
    COALESCE(m.payload:items, ARRAY_CONSTRUCT())::ARRAY AS items
  FROM messages AS m
  WHERE m.schema_version = 2
)

SELECT *
FROM final
