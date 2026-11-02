MODEL (
  description "One row per live partner sales channel listing with its commercial terms",
  columns (
    listing_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    listing_status (type VARCHAR),
    listed_at (type TIMESTAMP_TZ),
    commission_pct (type NUMBER(5, 2)),
    fulfilled_by_partner (type BOOLEAN),
    terms (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("partners__partner_listings")
),

@sql.cdc_latest(src, src.key := (listing_id))

final AS (
  SELECT
    l.listing_id,
    UPPER(l.partner_code)::VARCHAR AS partner_code,
    @sql.normalise_sku(l.sku) AS sku,
    LOWER(l.listing_status)::VARCHAR AS listing_status,
    l.listed_at,
    l.terms:commission_pct::NUMBER(5, 2) AS commission_pct,
    COALESCE(l.terms:fulfilment::VARCHAR = 'partner', FALSE) AS fulfilled_by_partner,
    l.terms::OBJECT AS terms,
    @sql.load_metadata(l)
  FROM cdc_latest AS l
)
SELECT *
FROM final
