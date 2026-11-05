TEST (name "dim_customers__combines_profile_value_consent_and_loyalty");

WITH __ref__stg_customers AS (
  SELECT
    'C1' AS customer_id,
    NULL AS email_normalised,
    NULL AS phone_normalised,
    NULL AS first_name,
    NULL AS last_name,
    'EU' AS region_code,
    'eu-web' AS storefront_code,
    NULL AS created_at,
    'de' AS preferred_language,
    TRUE AS marketing_opt_in,
    NULL AS consent_types,
    1990 AS birth_year,
    NULL AS preferences,
    NULL AS source_seq,
    NULL AS source_loaded_at
  UNION ALL
  SELECT
    'C2',
    NULL,
    NULL,
    NULL,
    NULL,
    'US',
    'us-web',
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL
  UNION ALL
  SELECT
    'C3',
    NULL,
    NULL,
    NULL,
    NULL,
    'EU',
    'eu-web',
    NULL,
    NULL,
    TRUE,
    NULL,
    2005,
    NULL,
    NULL,
    NULL
),

__ref__int_customer_identity AS (
  SELECT
    'C1' AS customer_id,
    NULL AS email_normalised,
    'example.com' AS email_domain,
    NULL AS email_dedupe_key,
    NULL AS phone_normalised,
    'Wren Featherby' AS full_name,
    NULL AS anonymous_ids,
    NULL AS anonymous_id_count,
    NULL AS newsletter_lists,
    NULL AS ticket_count,
    'C3' AS duplicate_candidate_id,
    NULL AS duplicate_name_similarity,
    NULL AS accounts_sharing_email
),

__ref__int_customer_address_current AS (
  SELECT
    'C1' AS customer_id,
    NULL AS address_id,
    'Munich' AS city,
    NULL AS postcode_canonical,
    'DE' AS country_code,
    'EU' AS region_code,
    NULL AS latitude,
    NULL AS longitude,
    NULL AS is_explicit_default,
    NULL AS address_count,
    NULL AS distinct_countries
),

__ref__int_customer_rfm AS (
  SELECT
    'C1' AS customer_id,
    NULL AS last_order_date,
    3 AS days_since_last_order,
    5 AS order_count,
    250.5::NUMBER(18, 2) AS lifetime_revenue_usd,
    NULL AS average_order_usd,
    NULL AS recency_score,
    NULL AS frequency_score,
    NULL AS monetary_score,
    NULL AS rfm_code,
    'champion' AS segment,
    TRUE AS is_vip
),

__ref__int_customer_consent_state AS (
  SELECT
    'C1' AS customer_id,
    FALSE AS email_marketing,
    TRUE AS sms_marketing,
    NULL AS personalisation,
    NULL AS third_party_sharing,
    NULL AS consent_state,
    NULL AS last_consent_change_at,
    NULL AS has_legacy_consent
),

__ref__int_loyalty_tier_changes AS (
  SELECT
    'LA1:1' AS tier_change_key,
    NULL AS loyalty_account_id,
    'C1' AS customer_id,
    1 AS change_sequence,
    'DUCKLING' AS tier_code,
    NULL AS previous_tier_code,
    NULL AS changed_at,
    NULL AS direction,
    NULL AS days_in_previous_tier
  UNION ALL
  SELECT
    'LA1:2',
    NULL,
    'C1',
    2,
    'GOLDEN_DUCK',
    NULL,
    NULL,
    NULL,
    NULL
),

__ref__int_cross_device_identity AS (
  SELECT
    'C1' AS customer_id,
    NULL AS web_visitor_count,
    NULL AS app_device_count,
    NULL AS platforms,
    TRUE AS is_multi_device,
    NULL AS first_seen_at_utc,
    NULL AS identity_graph
),

__expected__dim_customers AS (
  SELECT
    'C1' AS customer_id,
    'Munich' AS city,
    'EU' AS region_code,
    'Wren Featherby' AS full_name,
    '25_39' AS age_band,
    5 AS order_count,
    250.5::DECIMAL(18, 2) AS lifetime_revenue_usd,
    'champion' AS rfm_segment,
    TRUE AS is_vip,
    FALSE AS email_marketing,
    TRUE AS sms_marketing,
    'GOLDEN_DUCK' AS loyalty_tier_code,
    TRUE AS is_multi_device,
    'C3' AS possible_duplicate_of
  UNION ALL
  SELECT
    'C2' AS customer_id,
    CAST(NULL AS VARCHAR) AS city,
    'US' AS region_code,
    CAST(NULL AS VARCHAR) AS full_name,
    'unknown' AS age_band,
    0 AS order_count,
    0.0::DECIMAL(18, 2) AS lifetime_revenue_usd,
    'prospect' AS rfm_segment,
    FALSE AS is_vip,
    FALSE AS email_marketing,
    FALSE AS sms_marketing,
    CAST(NULL AS VARCHAR) AS loyalty_tier_code,
    FALSE AS is_multi_device,
    CAST(NULL AS VARCHAR) AS possible_duplicate_of
  UNION ALL
  SELECT
    'C3' AS customer_id,
    CAST(NULL AS VARCHAR) AS city,
    'EU' AS region_code,
    CAST(NULL AS VARCHAR) AS full_name,
    'under_25' AS age_band,
    0 AS order_count,
    0.0::DECIMAL(18, 2) AS lifetime_revenue_usd,
    'prospect' AS rfm_segment,
    FALSE AS is_vip,
    TRUE AS email_marketing,
    FALSE AS sms_marketing,
    CAST(NULL AS VARCHAR) AS loyalty_tier_code,
    FALSE AS is_multi_device,
    CAST(NULL AS VARCHAR) AS possible_duplicate_of
),

__assert__customer_id_is_unique_and_present AS (
  SELECT t.customer_id
  FROM __ref("dim_customers") AS t
  GROUP BY
    t.customer_id
  HAVING
    COUNT(*) > 1
    OR t.customer_id IS NULL
)
