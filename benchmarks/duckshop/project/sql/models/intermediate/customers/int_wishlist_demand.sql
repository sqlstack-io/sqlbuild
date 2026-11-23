MODEL (
  description """
    One row per SKU with how many customers have it on a wishlist and how many of them later bought
    it
  """,
  columns (
    sku (type VARCHAR, nullable false, audits [not_null, unique]),
    wishlisting_customers (type INT),
    public_wishlist_count (type INT),
    purchased_after_wishlist (type INT),
    wishlist_conversion_rate (type NUMBER(9, 6)),
    median_days_to_purchase (type NUMBER(10, 2)),
    first_added_at (type TIMESTAMP_TZ),
  ),
);

WITH wishlist_items AS (
  SELECT *
  FROM __ref("stg_wishlist_items")
),

wishlists AS (
  SELECT *
  FROM __ref("stg_wishlists")
),

order_lines AS (
  SELECT *
  FROM __ref("stg_order_lines")
),

orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

saved AS (
  SELECT
    i.sku,
    i.customer_id,
    MIN(i.added_at) AS added_at,
    MAX(IFF(w.is_public, 1, 0)) AS on_public_list
  FROM wishlist_items AS i
  INNER JOIN wishlists AS w
    ON i.wishlist_id = w.wishlist_id
  GROUP BY
    i.sku,
    i.customer_id
),

purchases AS (
  SELECT
    l.sku,
    o.customer_id,
    MIN(o.placed_at) AS first_purchased_at
  FROM order_lines AS l
  INNER JOIN orders AS o
    ON l.order_id = o.order_id
  GROUP BY
    l.sku,
    o.customer_id
),

matched AS (
  SELECT
    s.sku,
    s.customer_id,
    s.added_at,
    s.on_public_list,
    p.first_purchased_at,
    IFF(
      p.first_purchased_at >= s.added_at,
      DATEDIFF('hour', s.added_at, p.first_purchased_at) / 24,
      NULL
    ) AS days_to_purchase
  FROM saved AS s
  LEFT JOIN purchases AS p
    ON s.sku = p.sku
    AND s.customer_id = p.customer_id
),

final AS (
  SELECT
    m.sku,
    COUNT(DISTINCT m.customer_id)::INT AS wishlisting_customers,
    SUM(m.on_public_list)::INT AS public_wishlist_count,
    COUNT_IF(m.first_purchased_at >= m.added_at)::INT AS purchased_after_wishlist,
    ROUND(
      COUNT_IF(m.first_purchased_at >= m.added_at) / NULLIF(COUNT(DISTINCT m.customer_id), 0),
      6
    )::NUMBER(9, 6) AS wishlist_conversion_rate,
    MEDIAN(m.days_to_purchase)::NUMBER(10, 2) AS median_days_to_purchase,
    MIN(m.added_at) AS first_added_at
  FROM matched AS m
  GROUP BY
    m.sku
)

SELECT *
FROM final
