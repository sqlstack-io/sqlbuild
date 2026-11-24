MODEL (
  description """
    One row per creator with posts, reach, credited orders and revenue, ranked by credited revenue
    per platform
  """,
  columns (
    creator_handle (type VARCHAR, nullable false, audits [not_null, unique]),
    primary_platform (type VARCHAR),
    posts (type INT),
    impressions (type INT),
    engagements (type INT),
    credited_orders (type INT),
    credited_revenue_usd (type NUMBER(18, 2)),
    discount_given_usd (type NUMBER(18, 2)),
    revenue_rank (type INT),
  ),
);

WITH posts AS (
  SELECT *
  FROM __ref("int_influencer_promo_orders")
),

creators AS (
  SELECT
    p.creator_handle,
    MODE(p.platform) AS primary_platform,
    COUNT(*) AS posts,
    SUM(p.impressions) AS impressions,
    SUM(p.engagements) AS engagements,
    SUM(p.credited_orders) AS credited_orders,
    SUM(p.credited_revenue_usd) AS credited_revenue_usd,
    SUM(p.discount_given_usd) AS discount_given_usd
  FROM posts AS p
  GROUP BY
    p.creator_handle
),

final AS (
  SELECT
    c.creator_handle,
    c.primary_platform::VARCHAR AS primary_platform,
    c.posts::INT AS posts,
    c.impressions::INT AS impressions,
    c.engagements::INT AS engagements,
    c.credited_orders::INT AS credited_orders,
    c.credited_revenue_usd::NUMBER(18, 2) AS credited_revenue_usd,
    c.discount_given_usd::NUMBER(18, 2) AS discount_given_usd,
    RANK() OVER (
      PARTITION BY c.primary_platform
      ORDER BY c.credited_revenue_usd DESC
    )::INT AS revenue_rank
  FROM creators AS c
)

SELECT *
FROM final
