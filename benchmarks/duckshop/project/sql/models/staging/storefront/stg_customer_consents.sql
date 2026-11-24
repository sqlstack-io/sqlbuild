MODEL (
  description """
    One row per live consent record. Customers created before the consent service store their
    consents inside the profile payload; those are carried as legacy rows when no explicit record of
    the same type exists.
  """,
  columns (
    consent_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    consent_type (type VARCHAR, audits [not_null]),
    is_granted (type BOOLEAN),
    captured_at (type TIMESTAMP_TZ),
    capture_surface (type VARCHAR),
    policy_version (type VARCHAR),
    is_legacy (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__customer_consents")
),

customers AS (
  SELECT *
  FROM __source("storefront__customers")
),

@sql.cdc_latest(src, src.key := (consent_id))

explicit_consents AS (
  SELECT
    c.consent_id,
    c.customer_id,
    UPPER(REGEXP_REPLACE(c.consent_type, '[^A-Za-z]+', '_')) AS consent_type,
    COALESCE(c.granted, FALSE) AS is_granted,
    c.captured_at,
    COALESCE(c.source_context:surface::VARCHAR, 'unknown') AS capture_surface,
    REGEXP_SUBSTR(
      c.source_context:policy_url::VARCHAR,
      'v([0-9]+(\\.[0-9]+)?)',
      1,
      1,
      'e',
      1
    ) AS policy_version,
    FALSE AS is_legacy
  FROM cdc_latest AS c
), legacy_profiles AS (
  SELECT
    p.customer_id,
    p.created_at,
    p.profile
  FROM customers AS p
  QUALIFY
    ROW_NUMBER() OVER (PARTITION BY p.customer_id ORDER BY p._seq DESC) = 1
    AND p._op <> 'D'
), legacy_consents AS (
  SELECT
    (
      'legacy:' || l.customer_id || ':' || UPPER(f.value::VARCHAR)
    ) AS consent_id,
    l.customer_id,
    UPPER(REGEXP_REPLACE(
      f.value::VARCHAR,
      '[^A-Za-z]+',
      '_'
    )) AS consent_type,
    TRUE AS is_granted,
    l.created_at AS captured_at,
    'profile' AS capture_surface,
    NULL AS policy_version,
    TRUE AS is_legacy
  FROM legacy_profiles AS l, LATERAL FLATTEN(input => l.profile:consents) AS f
), legacy_only AS (
  SELECT
    lc.consent_id,
    lc.customer_id,
    lc.consent_type,
    lc.is_granted,
    lc.captured_at,
    lc.capture_surface,
    lc.policy_version,
    lc.is_legacy
  FROM legacy_consents AS lc
  LEFT JOIN explicit_consents AS ec
    ON lc.customer_id = ec.customer_id
    AND lc.consent_type = ec.consent_type
  WHERE ec.consent_id IS NULL
), final AS (
  SELECT
    e.consent_id,
    e.customer_id,
    e.consent_type::VARCHAR AS consent_type,
    e.is_granted,
    e.captured_at,
    e.capture_surface::VARCHAR AS capture_surface,
    e.policy_version::VARCHAR AS policy_version,
    e.is_legacy
  FROM explicit_consents AS e
  UNION ALL
  SELECT
    l.consent_id::VARCHAR AS consent_id,
    l.customer_id,
    l.consent_type::VARCHAR AS consent_type,
    l.is_granted,
    l.captured_at,
    l.capture_surface::VARCHAR AS capture_surface,
    l.policy_version::VARCHAR AS policy_version,
    l.is_legacy
  FROM legacy_only AS l
)
SELECT *
FROM final
