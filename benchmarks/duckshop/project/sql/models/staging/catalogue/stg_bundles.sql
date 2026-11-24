MODEL (
  description "One row per live product bundle and its pricing rule",
  columns (
    bundle_sku (type VARCHAR, nullable false, audits [not_null, unique]),
    title (type VARCHAR),
    active_from (type DATE),
    active_to (type DATE),
    pricing_mode (type VARCHAR, audits [
      accepted_values (values ["fixed", "percent_off", "cheapest_free"]),
    ]),
    fixed_amount_minor (type INT),
    percent_off (type NUMBER(5, 2)),
    pricing_currency_code (type VARCHAR),
    pricing (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__bundles")
),

@sql.cdc_latest(src, src.key := (bundle_sku))

final AS (
  SELECT
    @sql.normalise_sku(b.bundle_sku) AS bundle_sku,
    b.title,
    b.active_from,
    b.active_to,
    LOWER(COALESCE(b.pricing:mode::VARCHAR, 'fixed'))::VARCHAR AS pricing_mode,
    b.pricing:amount_minor::INT AS fixed_amount_minor,
    b.pricing:percent_off::NUMBER(5, 2) AS percent_off,
    UPPER(b.pricing:currency::VARCHAR)::VARCHAR AS pricing_currency_code,
    b.pricing::OBJECT AS pricing,
    @sql.load_metadata(b)
  FROM cdc_latest AS b
)
SELECT *
FROM final
