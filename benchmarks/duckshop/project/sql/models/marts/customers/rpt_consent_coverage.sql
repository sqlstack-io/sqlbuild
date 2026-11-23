MODEL (
  description "One row per region with the share of customers holding each marketing consent",
  columns (
    region_code (type VARCHAR, nullable false, audits [not_null, unique]),
    customers (type INT),
    email_marketing_share (type NUMBER(9, 4)),
    sms_marketing_share (type NUMBER(9, 4)),
    personalisation_share (type NUMBER(9, 4)),
    third_party_share (type NUMBER(9, 4)),
    legacy_consent_customers (type INT),
  ),
);

WITH customers AS (
  SELECT *
  FROM __ref("dim_customers")
),

consents AS (
  SELECT *
  FROM __ref("int_customer_consent_state")
),

joined AS (
  SELECT
    COALESCE(c.region_code, 'UNKNOWN') AS region_code,
    COALESCE(s.email_marketing, FALSE) AS email_marketing,
    COALESCE(s.sms_marketing, FALSE) AS sms_marketing,
    COALESCE(s.personalisation, FALSE) AS personalisation,
    COALESCE(s.third_party_sharing, FALSE) AS third_party_sharing,
    COALESCE(s.has_legacy_consent, FALSE) AS has_legacy_consent
  FROM customers AS c
  LEFT JOIN consents AS s
    ON c.customer_id = s.customer_id
),

final AS (
  SELECT
    j.region_code::VARCHAR AS region_code,
    COUNT(*)::INT AS customers,
    ROUND(AVG(IFF(j.email_marketing, 1, 0)), 4)::NUMBER(9, 4) AS email_marketing_share,
    ROUND(AVG(IFF(j.sms_marketing, 1, 0)), 4)::NUMBER(9, 4) AS sms_marketing_share,
    ROUND(AVG(IFF(j.personalisation, 1, 0)), 4)::NUMBER(9, 4) AS personalisation_share,
    ROUND(AVG(IFF(j.third_party_sharing, 1, 0)), 4)::NUMBER(9, 4) AS third_party_share,
    COUNT_IF(j.has_legacy_consent)::INT AS legacy_consent_customers
  FROM joined AS j
  GROUP BY
    j.region_code
)

SELECT *
FROM final
