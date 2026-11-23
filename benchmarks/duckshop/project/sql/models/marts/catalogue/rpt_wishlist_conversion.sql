MODEL (
  description "One row per brand with wishlist demand and how much of it converted to purchases",
  columns (
    brand_name (type VARCHAR, nullable false, audits [not_null, unique]),
    wishlisted_skus (type INT),
    wishlisting_customers (type INT),
    purchased_after_wishlist (type INT),
    conversion_rate (type NUMBER(9, 6)),
    median_days_to_purchase (type NUMBER(10, 2)),
  ),
);

WITH demand AS (
  SELECT *
  FROM __ref("int_wishlist_demand")
),

products AS (
  SELECT *
  FROM __ref("dim_products")
),

joined AS (
  SELECT
    COALESCE(p.brand_name, 'Unbranded') AS brand_name,
    d.sku,
    d.wishlisting_customers,
    d.purchased_after_wishlist,
    d.median_days_to_purchase
  FROM demand AS d
  LEFT JOIN products AS p
    ON d.sku = p.sku
),

final AS (
  SELECT
    j.brand_name::VARCHAR AS brand_name,
    COUNT(DISTINCT j.sku)::INT AS wishlisted_skus,
    SUM(j.wishlisting_customers)::INT AS wishlisting_customers,
    SUM(j.purchased_after_wishlist)::INT AS purchased_after_wishlist,
    ROUND(
      SUM(j.purchased_after_wishlist) / NULLIF(SUM(j.wishlisting_customers), 0),
      6
    )::NUMBER(9, 6) AS conversion_rate,
    ROUND(MEDIAN(j.median_days_to_purchase), 2)::NUMBER(10, 2) AS median_days_to_purchase
  FROM joined AS j
  GROUP BY
    j.brand_name
)

SELECT *
FROM final
