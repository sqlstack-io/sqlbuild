MODEL (
  description """
    One row per customer with the current state of each consent type, pivoted into columns, and the
    full state as an object for downstream audit.
  """,
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    email_marketing (type BOOLEAN),
    sms_marketing (type BOOLEAN),
    personalisation (type BOOLEAN),
    third_party_sharing (type BOOLEAN),
    consent_state (type OBJECT),
    last_consent_change_at (type TIMESTAMP_TZ),
    has_legacy_consent (type BOOLEAN),
  ),
);

WITH consents AS (
  SELECT *
  FROM __ref("stg_customer_consents")
),

current_consents AS (
  SELECT
    c.customer_id,
    c.consent_type,
    c.is_granted,
    c.captured_at,
    c.is_legacy
  FROM consents AS c
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY c.customer_id, c.consent_type
      ORDER BY c.captured_at DESC, c.consent_id DESC
    ) = 1
),

consent_matrix AS (
  SELECT
    cc.customer_id,
    cc.consent_type,
    cc.is_granted
  FROM current_consents AS cc
),

pivoted AS (
  SELECT
    p.customer_id,
    p.email_marketing,
    p.sms_marketing,
    p.personalisation,
    p.third_party_sharing
  FROM consent_matrix PIVOT(
    MAX(is_granted) FOR consent_type IN (
      'EMAIL_MARKETING',
      'SMS_MARKETING',
      'PERSONALISATION',
      'THIRD_PARTY_SHARING'
    )
  ) AS p(customer_id, email_marketing, sms_marketing, personalisation, third_party_sharing)
),

summary AS (
  SELECT
    cc.customer_id,
    OBJECT_AGG(cc.consent_type, cc.is_granted::VARIANT) AS consent_state,
    MAX(cc.captured_at) AS last_consent_change_at,
    BOOLOR_AGG(cc.is_legacy) AS has_legacy_consent
  FROM current_consents AS cc
  GROUP BY
    cc.customer_id
),

final AS (
  SELECT
    s.customer_id,
    COALESCE(p.email_marketing, FALSE)::BOOLEAN AS email_marketing,
    COALESCE(p.sms_marketing, FALSE)::BOOLEAN AS sms_marketing,
    COALESCE(p.personalisation, FALSE)::BOOLEAN AS personalisation,
    COALESCE(p.third_party_sharing, FALSE)::BOOLEAN AS third_party_sharing,
    s.consent_state::OBJECT AS consent_state,
    s.last_consent_change_at,
    s.has_legacy_consent::BOOLEAN AS has_legacy_consent
  FROM summary AS s
  LEFT JOIN pivoted AS p
    ON s.customer_id = p.customer_id
)

SELECT *
FROM final
