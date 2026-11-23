MODEL (
  description "One row per live carrier rate card lane, with its weight bands kept as an array",
  columns (
    rate_card_id (type VARCHAR, nullable false, audits [not_null, unique]),
    carrier_code (type VARCHAR, audits [not_null]),
    service_level (type VARCHAR),
    origin_country (type VARCHAR),
    destination_country (type VARCHAR),
    currency_code (type VARCHAR),
    valid_from (type DATE),
    valid_to (type DATE),
    band_count (type INT),
    bands (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__carrier_rate_cards")
),

@sql.cdc_latest(src, src.key := (rate_card_id))

final AS (
  SELECT
    r.rate_card_id,
    UPPER(r.carrier_code)::VARCHAR AS carrier_code,
    LOWER(r.service_level)::VARCHAR AS service_level,
    UPPER(r.origin_country)::VARCHAR AS origin_country,
    UPPER(r.destination_country)::VARCHAR AS destination_country,
    UPPER(r.currency_code)::VARCHAR AS currency_code,
    r.valid_from,
    COALESCE(r.valid_to, '9999-12-31'::DATE) AS valid_to,
    ARRAY_SIZE(r.bands)::INT AS band_count,
    r.bands::ARRAY AS bands,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
