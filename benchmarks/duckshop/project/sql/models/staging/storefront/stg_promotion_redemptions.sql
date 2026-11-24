MODEL (
  description "One row per live promotion redemption on an order",
  columns (
    redemption_id (type VARCHAR, nullable false, audits [not_null, unique]),
    promo_code (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR, audits [not_null]),
    discount_minor (type INT),
    redeemed_at (type TIMESTAMP_TZ),
    entry_method (type VARCHAR),
    redemption_context (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__promotion_redemptions")
),

@sql.cdc_latest(src, src.key := (redemption_id))

final AS (
  SELECT
    r.redemption_id,
    UPPER(TRIM(r.promo_code))::VARCHAR AS promo_code,
    r.order_id,
    ABS(COALESCE(r.discount_minor, 0))::INT AS discount_minor,
    r.redeemed_at,
    LOWER(COALESCE(r.context:entry::VARCHAR, 'code'))::VARCHAR AS entry_method,
    r.context::OBJECT AS redemption_context,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
