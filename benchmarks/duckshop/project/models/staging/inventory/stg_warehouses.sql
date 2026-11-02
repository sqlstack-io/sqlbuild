MODEL (
  description "One row per live warehouse with its IANA time zone and opening hours",
  columns (
    warehouse_code (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_name (type VARCHAR),
    country_code (type VARCHAR, audits [not_null]),
    iana_timezone (type VARCHAR, audits [not_null]),
    opens_local (type TIME),
    closes_local (type TIME),
    opening_minutes (type INT),
    supports_same_day (type BOOLEAN),
    supports_cold_chain (type BOOLEAN),
    max_parcel_kg (type NUMBER(6, 2)),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__warehouses")
),

@sql.cdc_latest(src, src.key := (warehouse_code))

final AS (
  SELECT
    UPPER(w.warehouse_code)::VARCHAR AS warehouse_code,
    w.warehouse_name,
    UPPER(w.country_code)::VARCHAR AS country_code,
    w.iana_timezone,
    w.opens_local,
    w.closes_local,
    DATEDIFF('minute', w.opens_local, w.closes_local)::INT AS opening_minutes,
    ARRAY_CONTAINS(
      'same_day'::VARIANT,
      COALESCE(w.capabilities:services, ARRAY_CONSTRUCT())::ARRAY
    )::BOOLEAN AS supports_same_day,
    COALESCE(w.capabilities:cold_chain::BOOLEAN, FALSE) AS supports_cold_chain,
    w.capabilities:limits.max_parcel_kg::NUMBER(6, 2) AS max_parcel_kg,
    @sql.load_metadata(w)
  FROM cdc_latest AS w
)
SELECT *
FROM final
