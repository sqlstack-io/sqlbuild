MODEL (
  description """
    One row per customer account with their current address, value segment, marketing consent,
    loyalty tier and device footprint. The marketing holdout flag samples customers with HASH, whose
    output is engine-specific, so the holdout set is an expected divergence across engines.
  """,
  tags [known_divergence],
  contract enforced,
  model_schema address,
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    email_domain (type VARCHAR),
    full_name (type VARCHAR),
    storefront_code (type VARCHAR),
    preferred_language (type VARCHAR),
    created_at (type TIMESTAMP_TZ),
    age_band (type VARCHAR),
    order_count (type INT),
    lifetime_revenue_usd (type NUMBER(18, 2), audits [non_negative]),
    days_since_last_order (type INT),
    rfm_segment (type VARCHAR),
    is_vip (type BOOLEAN),
    email_marketing (type BOOLEAN),
    sms_marketing (type BOOLEAN),
    loyalty_tier_code (type VARCHAR),
    is_multi_device (type BOOLEAN),
    possible_duplicate_of (type VARCHAR),
    is_marketing_holdout (type BOOLEAN),
  ),
);

WITH customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

identities AS (
  SELECT *
  FROM __ref("int_customer_identity")
),

addresses AS (
  SELECT *
  FROM __ref("int_customer_address_current")
),

rfm AS (
  SELECT *
  FROM __ref("int_customer_rfm")
),

consents AS (
  SELECT *
  FROM __ref("int_customer_consent_state")
),

tier_changes AS (
  SELECT *
  FROM __ref("int_loyalty_tier_changes")
),

devices AS (
  SELECT *
  FROM __ref("int_cross_device_identity")
),

current_tiers AS (
  SELECT
    t.customer_id,
    t.tier_code
  FROM tier_changes AS t
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY t.customer_id
      ORDER BY t.change_sequence DESC, t.tier_change_key DESC
    ) = 1
),

final AS (
  SELECT
    c.customer_id,
    a.city,
    a.postcode_canonical,
    a.country_code,
    COALESCE(a.region_code, c.region_code)::VARCHAR AS region_code,
    i.email_domain,
    i.full_name,
    c.storefront_code,
    c.preferred_language,
    c.created_at,
    CASE
      WHEN c.birth_year IS NULL
      THEN 'unknown'
      WHEN YEAR('@@valuation_date'::DATE) - c.birth_year < 25
      THEN 'under_25'
      WHEN YEAR('@@valuation_date'::DATE) - c.birth_year < 40
      THEN '25_39'
      WHEN YEAR('@@valuation_date'::DATE) - c.birth_year < 60
      THEN '40_59'
      ELSE '60_plus'
    END::VARCHAR AS age_band,
    COALESCE(r.order_count, 0)::INT AS order_count,
    COALESCE(r.lifetime_revenue_usd, 0)::NUMBER(18, 2) AS lifetime_revenue_usd,
    r.days_since_last_order,
    COALESCE(r.segment, 'prospect')::VARCHAR AS rfm_segment,
    COALESCE(r.is_vip, FALSE)::BOOLEAN AS is_vip,
    COALESCE(s.email_marketing, c.marketing_opt_in, FALSE)::BOOLEAN AS email_marketing,
    COALESCE(s.sms_marketing, FALSE)::BOOLEAN AS sms_marketing,
    t.tier_code AS loyalty_tier_code,
    COALESCE(d.is_multi_device, FALSE)::BOOLEAN AS is_multi_device,
    i.duplicate_candidate_id AS possible_duplicate_of,
    (
      MOD(ABS(HASH(c.customer_id)), 100) < 5
    )::BOOLEAN AS is_marketing_holdout
  FROM customers AS c
  LEFT JOIN identities AS i
    ON c.customer_id = i.customer_id
  LEFT JOIN addresses AS a
    ON c.customer_id = a.customer_id
  LEFT JOIN rfm AS r
    ON c.customer_id = r.customer_id
  LEFT JOIN consents AS s
    ON c.customer_id = s.customer_id
  LEFT JOIN current_tiers AS t
    ON c.customer_id = t.customer_id
  LEFT JOIN devices AS d
    ON c.customer_id = d.customer_id
)

SELECT *
FROM final
