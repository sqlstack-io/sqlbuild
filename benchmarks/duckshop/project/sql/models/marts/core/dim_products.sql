MODEL (
  description """
    One row per sellable SKU with its product, brand and category attributes, current USD list price
    and margin, primary supplier, and review and wishlist signals.
  """,
  contract enforced,
  model_schema audit_ts,
  columns (
    sku (type VARCHAR, nullable false, audits [not_null, unique]),
    product_id (type VARCHAR, audits [not_null]),
    sku_family (type VARCHAR),
    title (type VARCHAR),
    variant_label (type VARCHAR),
    brand_name (type VARCHAR),
    is_private_label (type BOOLEAN),
    category_level_1 (type VARCHAR),
    category_slug_path (type VARCHAR),
    is_active (type BOOLEAN),
    launched_on (type DATE),
    list_price_usd (type NUMBER(18, 4), audits [non_negative]),
    margin_pct (type NUMBER(9, 2)),
    primary_supplier_code (type VARCHAR),
    supplier_count (type INT),
    review_count (type INT),
    average_rating (type NUMBER(4, 2)),
    wishlisting_customers (type INT),
  ),
);

WITH catalogue AS (
  SELECT *
  FROM __ref("int_product_catalogue")
),

products AS (
  SELECT *
  FROM __ref("stg_products")
),

margins AS (
  SELECT *
  FROM __ref("int_product_margin")
),

supplier_costs AS (
  SELECT *
  FROM __ref("int_supplier_cost_current")
),

reviews AS (
  SELECT *
  FROM __ref("int_reviews_moderated")
),

wishlists AS (
  SELECT *
  FROM __ref("int_wishlist_demand")
),

usd_margins AS (
  SELECT
    m.sku,
    m.list_price,
    m.margin_pct
  FROM margins AS m
  WHERE m.currency_code = 'USD'
),

review_stats AS (
  SELECT
    r.sku,
    COUNT(*) AS review_count,
    AVG(r.rating) AS average_rating
  FROM reviews AS r
  GROUP BY
    r.sku
),

final AS (
  SELECT
    c.sku,
    c.product_id,
    c.sku_family,
    c.title,
    c.variant_label,
    c.brand_name,
    c.is_private_label,
    c.category_level_1,
    c.category_slug_path,
    c.is_active,
    c.launched_on,
    m.list_price AS list_price_usd,
    m.margin_pct,
    s.supplier_code AS primary_supplier_code,
    COALESCE(s.supplier_count, 0)::INT AS supplier_count,
    COALESCE(r.review_count, 0)::INT AS review_count,
    ROUND(r.average_rating, 2)::NUMBER(4, 2) AS average_rating,
    COALESCE(w.wishlisting_customers, 0)::INT AS wishlisting_customers,
    p.source_loaded_at
  FROM catalogue AS c
  INNER JOIN products AS p
    ON c.product_id = p.product_id
  LEFT JOIN usd_margins AS m
    ON c.sku = m.sku
  LEFT JOIN supplier_costs AS s
    ON c.sku = s.sku
  LEFT JOIN review_stats AS r
    ON c.sku = r.sku
  LEFT JOIN wishlists AS w
    ON c.sku = w.sku
)

SELECT *
FROM final
