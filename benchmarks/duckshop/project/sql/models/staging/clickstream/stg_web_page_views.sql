MODEL (
  description "One row per human page view, with the previous page of the same visitor",
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    viewed_at_utc (type TIMESTAMP_NTZ, audits [not_null]),
    page_path (type VARCHAR),
    page_type (type VARCHAR),
    previous_page_path (type VARCHAR),
    utm_source (type VARCHAR),
    utm_medium (type VARCHAR),
    utm_campaign (type VARCHAR),
    referrer_host (type VARCHAR),
    page_title (type VARCHAR),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

page_views AS (
  SELECT
    e.event_id,
    e.anonymous_id,
    e.customer_id,
    e.event_at_utc,
    e.page_url,
    e.page_path,
    e.referrer_host,
    e.properties:title::VARCHAR AS page_title
  FROM web_events AS e
  WHERE
    e.event_type = 'page_view'
    AND NOT e.is_bot
    AND e.anonymous_id IS NOT NULL
),

final AS (
  SELECT
    p.event_id,
    p.anonymous_id,
    p.customer_id,
    p.event_at_utc AS viewed_at_utc,
    p.page_path,
    CASE
      WHEN p.page_path RLIKE '^/products?/.*'
      THEN 'product'
      WHEN p.page_path RLIKE '^/(c|category)/.*'
      THEN 'category'
      WHEN p.page_path RLIKE '^/checkout.*'
      THEN 'checkout'
      WHEN p.page_path IN ('/', '')
      THEN 'home'
      ELSE 'content'
    END::VARCHAR AS page_type,
    LAG(p.page_path) OVER (
      PARTITION BY p.anonymous_id
      ORDER BY p.event_at_utc, p.event_id
    )::VARCHAR AS previous_page_path,
    REGEXP_SUBSTR(p.page_url, '[?&]utm_source=([^&#]+)', 1, 1, 'e', 1)::VARCHAR AS utm_source,
    REGEXP_SUBSTR(p.page_url, '[?&]utm_medium=([^&#]+)', 1, 1, 'e', 1)::VARCHAR AS utm_medium,
    REGEXP_SUBSTR(p.page_url, '[?&]utm_campaign=([^&#]+)', 1, 1, 'e', 1)::VARCHAR AS utm_campaign,
    p.referrer_host,
    p.page_title::VARCHAR AS page_title
  FROM page_views AS p
)

SELECT *
FROM final
