MODEL (
  description """
    One row per approved or pending product review with a verified-purchase flag, a keyword
    sentiment score and its weight in the product's rating.
  """,
  constants (
    _positive_terms "(love|great|excellent|perfect|sturdy|beautiful)",
    _negative_terms "(broke|broken|cheap|flimsy|disappoint|return|rust)",
  ),
  columns (
    review_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    product_id (type VARCHAR),
    customer_id (type VARCHAR),
    rating (type INT),
    submitted_at (type TIMESTAMP_TZ),
    is_verified_purchase (type BOOLEAN),
    sentiment_score (type INT),
    review_weight (type NUMBER(6, 3)),
    rating_rank_in_product (type INT),
  ),
);

WITH reviews AS (
  SELECT *
  FROM __ref("stg_reviews")
),

order_lines AS (
  SELECT *
  FROM __ref("int_order_lines_priced")
),

orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

variants AS (
  SELECT *
  FROM __ref("stg_product_variants")
),

purchases AS (
  SELECT DISTINCT
    o.customer_id,
    l.sku
  FROM order_lines AS l
  INNER JOIN orders AS o
    ON l.order_id = o.order_id
),

visible AS (
  SELECT
    r.review_id,
    r.sku,
    r.customer_id,
    r.rating,
    r.title,
    r.submitted_at,
    r.body_length
  FROM reviews AS r
  WHERE
    r.moderation_status IN ('approved', 'pending')
    AND ARRAY_SIZE(r.flagged_terms) = 0
),

scored AS (
  SELECT
    v.review_id,
    REGEXP_COUNT(LOWER(v.title), @const("_positive_terms"))
      - REGEXP_COUNT(LOWER(v.title), @const("_negative_terms")) AS sentiment_score
  FROM visible AS v
),

final AS (
  SELECT
    v.review_id,
    v.sku,
    pv.product_id,
    v.customer_id,
    v.rating,
    v.submitted_at,
    (
      p.customer_id IS NOT NULL
    ) AS is_verified_purchase,
    s.sentiment_score::INT AS sentiment_score,
    ROUND(
      IFF(p.customer_id IS NOT NULL, 1.5, 1.0) * LEAST(1 + LN(1 + v.body_length / 100), 3),
      3
    )::NUMBER(6, 3) AS review_weight,
    RANK() OVER (PARTITION BY v.sku ORDER BY v.rating DESC)::INT AS rating_rank_in_product
  FROM visible AS v
  INNER JOIN scored AS s
    ON v.review_id = s.review_id
  LEFT JOIN purchases AS p
    ON v.customer_id = p.customer_id
    AND v.sku = p.sku
  LEFT JOIN variants AS pv
    ON v.sku = pv.sku
)

SELECT *
FROM final
