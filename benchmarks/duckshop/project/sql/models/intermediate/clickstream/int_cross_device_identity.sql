MODEL (
  description """
    One row per customer with the web visitors and app devices linked to them, the platforms they
    use, and a stable household-independent identity graph key.
  """,
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    web_visitor_count (type INT),
    app_device_count (type INT),
    platforms (type ARRAY),
    is_multi_device (type BOOLEAN),
    first_seen_at_utc (type TIMESTAMP_NTZ),
    identity_graph (type OBJECT),
  ),
);

WITH web_identities AS (
  SELECT *
  FROM __ref("stg_web_identifies")
),

app_events AS (
  SELECT *
  FROM __ref("stg_app_events")
),

web_links AS (
  SELECT
    w.customer_id::VARCHAR AS customer_id,
    w.anonymous_id::VARCHAR AS identifier,
    'web'::VARCHAR AS platform,
    w.first_seen_at_utc::TIMESTAMP_NTZ AS first_seen_at_utc
  FROM web_identities AS w
),

app_links AS (
  SELECT
    a.customer_id::VARCHAR AS customer_id,
    a.device_id::VARCHAR AS identifier,
    a.platform::VARCHAR AS platform,
    MIN(a.event_at_utc)::TIMESTAMP_NTZ AS first_seen_at_utc
  FROM app_events AS a
  WHERE
    a.customer_id IS NOT NULL
    AND a.device_id IS NOT NULL
  GROUP BY
    a.customer_id,
    a.device_id,
    a.platform
),

all_links AS (
  SELECT
    wl.customer_id::VARCHAR AS customer_id,
    wl.identifier::VARCHAR AS identifier,
    wl.platform::VARCHAR AS platform,
    wl.first_seen_at_utc::TIMESTAMP_NTZ AS first_seen_at_utc
  FROM web_links AS wl
  UNION ALL
  SELECT
    al.customer_id::VARCHAR AS customer_id,
    al.identifier::VARCHAR AS identifier,
    al.platform::VARCHAR AS platform,
    al.first_seen_at_utc::TIMESTAMP_NTZ AS first_seen_at_utc
  FROM app_links AS al
),

per_platform AS (
  SELECT
    l.customer_id,
    l.platform,
    ARRAY_AGG(DISTINCT l.identifier) WITHIN GROUP (ORDER BY l.identifier) AS identifiers,
    MIN(l.first_seen_at_utc) AS first_seen_at_utc
  FROM all_links AS l
  GROUP BY
    l.customer_id,
    l.platform
),

final AS (
  SELECT
    p.customer_id,
    SUM(IFF(p.platform = 'web', ARRAY_SIZE(p.identifiers), 0))::INT AS web_visitor_count,
    SUM(IFF(p.platform <> 'web', ARRAY_SIZE(p.identifiers), 0))::INT AS app_device_count,
    ARRAY_AGG(p.platform) WITHIN GROUP (ORDER BY p.platform)::ARRAY AS platforms,
    (
      SUM(ARRAY_SIZE(p.identifiers)) > 1
    ) AS is_multi_device,
    MIN(p.first_seen_at_utc) AS first_seen_at_utc,
    OBJECT_AGG(p.platform, p.identifiers::VARIANT)::OBJECT AS identity_graph
  FROM per_platform AS p
  GROUP BY
    p.customer_id
)

SELECT *
FROM final
