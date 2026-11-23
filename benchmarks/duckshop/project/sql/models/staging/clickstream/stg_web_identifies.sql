MODEL (
  description """
    One row per anonymous visitor and customer pair seen on web, with first and last sighting
  """,
  columns (
    identity_link_key (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR, audits [not_null]),
    first_seen_at_utc (type TIMESTAMP_NTZ),
    last_seen_at_utc (type TIMESTAMP_NTZ),
    event_count (type INT),
    identify_methods (type ARRAY),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

identified AS (
  SELECT
    e.anonymous_id,
    e.customer_id,
    e.event_at_utc,
    COALESCE(e.properties:method::VARCHAR, e.event_type) AS identify_method
  FROM web_events AS e
  WHERE
    e.anonymous_id IS NOT NULL
    AND e.customer_id IS NOT NULL
    AND e.event_type IN ('identify', 'login', 'signup', 'order_completed')
),

final AS (
  SELECT
    (
      i.anonymous_id || '>' || i.customer_id
    )::VARCHAR AS identity_link_key,
    i.anonymous_id,
    i.customer_id,
    MIN(i.event_at_utc) AS first_seen_at_utc,
    MAX(i.event_at_utc) AS last_seen_at_utc,
    COUNT(*)::INT AS event_count,
    ARRAY_AGG(DISTINCT i.identify_method) WITHIN GROUP (
      ORDER BY i.identify_method
    )::ARRAY AS identify_methods
  FROM identified AS i
  GROUP BY
    i.anonymous_id,
    i.customer_id
)

SELECT *
FROM final
