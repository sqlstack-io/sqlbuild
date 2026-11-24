MODEL (
  description "One row per live replenishment subscription",
  columns (
    subscription_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    plan_code (type VARCHAR),
    sku (type VARCHAR),
    interval_days (type INT),
    subscription_status (type VARCHAR),
    started_at (type TIMESTAMP_TZ),
    cancelled_at (type TIMESTAMP_TZ),
    billing_amount_minor (type INT),
    billing_currency_code (type VARCHAR),
    next_bill_on (type DATE),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__subscriptions")
),

@sql.cdc_latest(src, src.key := (subscription_id))

final AS (
  SELECT
    s.subscription_id,
    s.customer_id,
    UPPER(s.plan_code)::VARCHAR AS plan_code,
    @sql.normalise_sku(s.sku) AS sku,
    COALESCE(s.interval_days, 30)::INT AS interval_days,
    LOWER(s.subscription_status)::VARCHAR AS subscription_status,
    s.started_at,
    s.cancelled_at,
    s.billing:amount_minor::INT AS billing_amount_minor,
    UPPER(s.billing:currency::VARCHAR)::VARCHAR AS billing_currency_code,
    TRY_TO_DATE(
      s.billing:next_bill_on::VARCHAR,
      'YYYY-MM-DD'
    ) AS next_bill_on,
    s.billing::OBJECT AS billing,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
)
SELECT *
FROM final
