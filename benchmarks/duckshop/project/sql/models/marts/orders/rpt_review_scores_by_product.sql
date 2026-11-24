MODEL (
  description """
    One row per product with weighted review score, rating distribution and verified-purchase share
  """,
  columns (
    product_id (type VARCHAR, nullable false, audits [not_null, unique]),
    review_count (type INT),
    weighted_rating (type NUMBER(4, 2)),
    rating_distribution (type OBJECT),
    verified_share (type NUMBER(9, 4)),
    net_sentiment (type INT),
    latest_review_at (type TIMESTAMP_TZ),
  ),
);

WITH reviews AS (
  SELECT *
  FROM __ref("int_reviews_moderated")
),

rating_counts AS (
  SELECT
    r.product_id,
    r.rating,
    COUNT(*) AS rating_count
  FROM reviews AS r
  GROUP BY
    r.product_id,
    r.rating
),

distribution AS (
  SELECT
    c.product_id,
    OBJECT_AGG(c.rating::VARCHAR, c.rating_count::VARIANT) AS rating_distribution
  FROM rating_counts AS c
  GROUP BY
    c.product_id
),

summary AS (
  SELECT
    r.product_id,
    COUNT(*) AS review_count,
    SUM(r.rating * r.review_weight) / NULLIF(SUM(r.review_weight), 0) AS weighted_rating,
    AVG(IFF(r.is_verified_purchase, 1, 0)) AS verified_share,
    SUM(r.sentiment_score) AS net_sentiment,
    MAX(r.submitted_at) AS latest_review_at
  FROM reviews AS r
  GROUP BY
    r.product_id
),

final AS (
  SELECT
    s.product_id,
    s.review_count::INT AS review_count,
    ROUND(s.weighted_rating, 2)::NUMBER(4, 2) AS weighted_rating,
    d.rating_distribution::OBJECT AS rating_distribution,
    ROUND(s.verified_share, 4)::NUMBER(9, 4) AS verified_share,
    s.net_sentiment::INT AS net_sentiment,
    s.latest_review_at
  FROM summary AS s
  LEFT JOIN distribution AS d
    ON s.product_id = d.product_id
)

SELECT *
FROM final
