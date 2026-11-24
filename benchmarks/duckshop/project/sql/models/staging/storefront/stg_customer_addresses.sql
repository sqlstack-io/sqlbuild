MODEL (
  description "One row per live customer address with a canonical postcode and coordinates",
  columns (
    address_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    country_code (type VARCHAR, audits [not_null]),
    postcode_canonical (type VARCHAR),
    is_default (type BOOLEAN),
    valid_from (type TIMESTAMP_TZ),
    latitude (type NUMBER(9, 6)),
    longitude (type NUMBER(9, 6)),
    geo (type OBJECT),
    line1 (type VARCHAR),
    line2 (type VARCHAR),
    city (type VARCHAR),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__customer_addresses")
),

@sql.cdc_latest(src, src.key := (address_id))

final AS (
  SELECT
    a.address_id,
    a.customer_id,
    TRIM(a.line1)::VARCHAR AS line1,
    NULLIF(TRIM(a.line2), '')::VARCHAR AS line2,
    INITCAP(TRIM(a.city))::VARCHAR AS city,
    UPPER(a.country_code)::VARCHAR AS country_code,
    CASE UPPER(a.country_code)
      WHEN 'US'
      THEN LPAD(REGEXP_SUBSTR(a.postcode, '[0-9]{1,5}'), 5, '0')
      WHEN 'GB'
      THEN REGEXP_REPLACE(UPPER(TRIM(a.postcode)), '^([A-Z0-9]+)\\s*([0-9][A-Z]{2})$', '\\1 \\2')
      WHEN 'JP'
      THEN REGEXP_REPLACE(a.postcode, '^([0-9]{3})-?([0-9]{4})$', '\\1-\\2')
      ELSE UPPER(REGEXP_REPLACE(a.postcode, '\\s+', ''))
    END::VARCHAR AS postcode_canonical,
    COALESCE(a.is_default, FALSE) AS is_default,
    a.valid_from,
    ROUND(a.geo:lat::FLOAT, 6)::NUMBER(9, 6) AS latitude,
    ROUND(a.geo:lon::FLOAT, 6)::NUMBER(9, 6) AS longitude,
    a.geo::OBJECT AS geo,
    @sql.load_metadata(a)
  FROM cdc_latest AS a
)
SELECT *
FROM final
