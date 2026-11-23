MODEL (
  description """
    One row per creator post with the orders that used its promo code. When creators reuse a code,
    each order is credited to the latest post published before it, and orders more than the
    attribution window after that post are not credited.
  """,
  constants (
    _attribution_window_days 14,
  ),
  columns (
    post_id (type VARCHAR, nullable false, audits [not_null, unique]),
    creator_handle (type VARCHAR),
    platform (type VARCHAR),
    posted_at (type TIMESTAMP_TZ),
    promo_code (type VARCHAR),
    impressions (type INT),
    engagements (type INT),
    saves (type INT),
    has_tracking_link (type BOOLEAN),
    credited_orders (type INT),
    credited_customers (type INT),
    credited_revenue_usd (type NUMBER(18, 2)),
    discount_given_usd (type NUMBER(18, 2)),
    first_order_hours_after_post (type NUMBER(10, 2)),
    orders_per_thousand_impressions (type NUMBER(12, 4)),
  ),
);

WITH posts AS (
  SELECT *
  FROM __ref("stg_influencer_posts")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

post_codes AS (
  SELECT
    p.post_id,
    LOWER(REGEXP_REPLACE(p.creator_handle, '^@+|\\s+', '')) AS creator_handle,
    LOWER(p.platform) AS platform,
    p.posted_at,
    UPPER(p.promo_code) AS promo_code,
    p.impressions,
    COALESCE(p.likes, 0)
      + COALESCE(p.metrics:comments::INT, 0)
      + COALESCE(p.metrics:shares::INT, 0) AS engagements,
    p.metrics:saves::INT AS saves,
    REGEXP_LIKE(
      COALESCE(p.metrics:link_url::VARCHAR, ''),
      '^https?://[^ ]+[?&]utm_source=.*'
    ) AS has_tracking_link
  FROM posts AS p
  WHERE p.promo_code IS NOT NULL
),

promo_orders AS (
  SELECT
    o.order_id,
    o.customer_id,
    UPPER(o.promo_code) AS promo_code,
    o.placed_at,
    o.amount_usd,
    o.discount_amount_usd
  FROM orders AS o
  WHERE
    o.promo_code IS NOT NULL
    AND o.order_status <> 'CANCELLED'
),

credited AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.placed_at,
    o.amount_usd,
    o.discount_amount_usd,
    p.post_id,
    p.posted_at
  FROM promo_orders AS o
  ASOF JOIN post_codes AS p MATCH_CONDITION (o.placed_at >= p.posted_at)
    ON o.promo_code = p.promo_code
),

in_window AS (
  SELECT
    c.post_id,
    c.order_id,
    c.customer_id,
    c.amount_usd,
    c.discount_amount_usd,
    DATEDIFF('minute', c.posted_at, c.placed_at) / 60 AS hours_after_post
  FROM credited AS c
  WHERE
    c.post_id IS NOT NULL
    AND DATEDIFF('day', c.posted_at, c.placed_at) <= @const("_attribution_window_days")
),

per_post AS (
  SELECT
    w.post_id,
    COUNT(DISTINCT w.order_id) AS credited_orders,
    COUNT(DISTINCT w.customer_id) AS credited_customers,
    SUM(w.amount_usd) AS credited_revenue_usd,
    SUM(w.discount_amount_usd) AS discount_given_usd,
    MIN(w.hours_after_post) AS first_order_hours_after_post
  FROM in_window AS w
  GROUP BY
    w.post_id
),

final AS (
  SELECT
    p.post_id,
    p.creator_handle::VARCHAR AS creator_handle,
    p.platform::VARCHAR AS platform,
    p.posted_at,
    p.promo_code::VARCHAR AS promo_code,
    p.impressions,
    p.engagements::INT AS engagements,
    p.saves::INT AS saves,
    p.has_tracking_link::BOOLEAN AS has_tracking_link,
    COALESCE(s.credited_orders, 0)::INT AS credited_orders,
    COALESCE(s.credited_customers, 0)::INT AS credited_customers,
    COALESCE(s.credited_revenue_usd, 0)::NUMBER(18, 2) AS credited_revenue_usd,
    COALESCE(s.discount_given_usd, 0)::NUMBER(18, 2) AS discount_given_usd,
    ROUND(s.first_order_hours_after_post, 2)::NUMBER(10, 2) AS first_order_hours_after_post,
    ROUND(
      1000 * COALESCE(s.credited_orders, 0) / NULLIF(p.impressions, 0),
      4
    )::NUMBER(12, 4) AS orders_per_thousand_impressions
  FROM post_codes AS p
  LEFT JOIN per_post AS s
    ON p.post_id = s.post_id
)

SELECT *
FROM final
