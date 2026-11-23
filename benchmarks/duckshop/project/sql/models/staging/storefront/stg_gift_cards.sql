MODEL (
  description "One row per issued gift card",
  columns (
    gift_card_id (type VARCHAR, nullable false, audits [not_null, unique]),
    code_last4 (type VARCHAR),
    currency_code (type VARCHAR, audits [not_null]),
    initial_amount_minor (type INT, audits [not_null]),
    issued_at (type TIMESTAMP_TZ),
    expires_on (type DATE),
    purchaser_customer_id (type VARCHAR),
    is_promotional (type BOOLEAN),
    design_code (type VARCHAR),
    details (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__gift_cards")
),

@sql.cdc_latest(src, src.key := (gift_card_id))

final AS (
  SELECT
    g.gift_card_id,
    RIGHT(REGEXP_REPLACE(g.code_masked, '[^0-9A-Z]', ''), 4)::VARCHAR AS code_last4,
    UPPER(g.currency_code)::VARCHAR AS currency_code,
    g.initial_amount_minor,
    g.issued_at,
    g.expires_on,
    g.purchaser_customer_id,
    COALESCE(g.details:promotional::BOOLEAN, FALSE) AS is_promotional,
    COALESCE(@sql.json_text(g.details:design), 'standard')::VARCHAR AS design_code,
    g.details::OBJECT AS details,
    @sql.load_metadata(g)
  FROM cdc_latest AS g
)
SELECT *
FROM final
