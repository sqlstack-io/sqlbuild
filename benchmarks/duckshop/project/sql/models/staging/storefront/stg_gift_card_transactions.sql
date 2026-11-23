MODEL (
  description "One row per live gift card redemption, top-up or expiry",
  columns (
    transaction_id (type VARCHAR, nullable false, audits [not_null, unique]),
    gift_card_id (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    transaction_type (type VARCHAR, audits [
      accepted_values (values ["REDEEM", "TOP_UP", "EXPIRE", "REVERSAL"]),
    ]),
    signed_amount_minor (type INT, audits [not_null]),
    occurred_at (type TIMESTAMP_TZ),
    channel (type VARCHAR),
    terminal_id (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__gift_card_transactions")
),

@sql.cdc_latest(src, src.key := (transaction_id))

final AS (
  SELECT
    t.transaction_id,
    t.gift_card_id,
    t.order_id,
    UPPER(REPLACE(t.transaction_type, '-', '_'))::VARCHAR AS transaction_type,
    IFF(
      UPPER(t.transaction_type) IN ('REDEEM', 'EXPIRE'),
      -ABS(t.amount_minor),
      ABS(t.amount_minor)
    )::INT AS signed_amount_minor,
    t.occurred_at,
    LOWER(
      COALESCE(t.context:channel::VARCHAR, IFF(t.order_id IS NULL, 'store', 'web'))
    )::VARCHAR AS channel,
    @sql.json_text(t.context:terminal.id) AS terminal_id,
    @sql.load_metadata(t)
  FROM cdc_latest AS t
)
SELECT *
FROM final
