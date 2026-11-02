MODEL (
  description """
    One row per live customer account with normalised contact details and profile preferences
  """,
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    email_normalised (type VARCHAR),
    phone_normalised (type VARCHAR),
    first_name (type VARCHAR),
    last_name (type VARCHAR),
    region_code (type VARCHAR, audits [not_null]),
    storefront_code (type VARCHAR),
    created_at (type TIMESTAMP_TZ, audits [not_null]),
    preferred_language (type VARCHAR),
    marketing_opt_in (type BOOLEAN),
    consent_types (type ARRAY),
    birth_year (type INT),
    preferences (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__customers")
),

@sql.cdc_latest(src, src.key := (customer_id))

final AS (
  SELECT
    c.customer_id,
    @sql.normalise_email(c.email) AS email_normalised,
    @sql.normalise_phone(c.phone) AS phone_normalised,
    INITCAP(TRIM(SPLIT_PART(c.full_name, ' ', 1)))::VARCHAR AS first_name,
    INITCAP(TRIM(REGEXP_SUBSTR(c.full_name, '\\s(.+)$', 1, 1, 'e', 1)))::VARCHAR AS last_name,
    UPPER(c.region_code)::VARCHAR AS region_code,
    UPPER(c.storefront_code)::VARCHAR AS storefront_code,
    c.created_at,
    COALESCE(c.profile:preferences.language::VARCHAR, 'en')::VARCHAR AS preferred_language,
    COALESCE(c.profile:preferences.marketing_opt_in::BOOLEAN, FALSE) AS marketing_opt_in,
    COALESCE(c.profile:consents, ARRAY_CONSTRUCT())::ARRAY AS consent_types,
    TRY_TO_NUMBER(c.profile:birth_year::VARCHAR)::INT AS birth_year,
    c.profile:preferences::OBJECT AS preferences,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
