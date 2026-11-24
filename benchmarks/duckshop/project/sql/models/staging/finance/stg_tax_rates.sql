MODEL (
  description "One row per live tax rate for a country, region and product tax category",
  columns (
    tax_rate_id (type VARCHAR, nullable false, audits [not_null, unique]),
    country_code (type VARCHAR, audits [not_null]),
    tax_region (type VARCHAR),
    tax_category (type VARCHAR),
    rate_pct (type NUMBER(9, 4)),
    rate_fraction (type NUMBER(9, 6)),
    valid_from (type DATE),
    valid_to (type DATE),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("finance__tax_rates")
),

@sql.cdc_latest(src, src.key := (tax_rate_id))

final AS (
  SELECT
    t.tax_rate_id,
    UPPER(t.country_code)::VARCHAR AS country_code,
    COALESCE(UPPER(t.tax_region), '*')::VARCHAR AS tax_region,
    LOWER(t.tax_category)::VARCHAR AS tax_category,
    t.rate_pct,
    ROUND(t.rate_pct / 100, 6)::NUMBER(9, 6) AS rate_fraction,
    t.valid_from,
    COALESCE(t.valid_to, '9999-12-31'::DATE) AS valid_to,
    @sql.load_metadata(t)
  FROM cdc_latest AS t
)
SELECT *
FROM final
