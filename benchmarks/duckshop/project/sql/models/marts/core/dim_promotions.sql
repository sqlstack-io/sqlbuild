MODEL (
  description """
    One row per promotion code with its terms, eligible regions and lifetime redemption results
  """,
  contract enforced,
  model_schema audit_ts,
  columns (
    promo_code (type VARCHAR, nullable false, audits [not_null, unique]),
    promo_name (type VARCHAR),
    starts_at (type TIMESTAMP_TZ),
    ends_at (type TIMESTAMP_TZ),
    is_stackable (type BOOLEAN),
    min_order_minor (type INT),
    max_discount_pct (type NUMBER(5, 2)),
    eligible_regions (type ARRAY),
    is_active_now (type BOOLEAN),
    redemption_count (type INT),
    revenue_usd (type NUMBER(18, 2)),
    discount_rate (type NUMBER(9, 6)),
    basket_uplift_pct (type NUMBER(9, 2)),
  ),
);

WITH promotions AS (
  SELECT *
  FROM __ref("stg_promotions")
),

performance AS (
  SELECT *
  FROM __ref("int_promotion_performance")
),

final AS (
  SELECT
    p.promo_code,
    p.promo_name,
    p.starts_at,
    p.ends_at,
    p.is_stackable,
    p.min_order_minor,
    p.max_discount_pct,
    p.eligible_regions,
    COALESCE(f.is_active_now, FALSE)::BOOLEAN AS is_active_now,
    COALESCE(f.redemption_count, 0)::INT AS redemption_count,
    COALESCE(f.revenue_usd, 0)::NUMBER(18, 2) AS revenue_usd,
    f.discount_rate,
    f.basket_uplift_pct,
    p.source_loaded_at
  FROM promotions AS p
  LEFT JOIN performance AS f
    ON p.promo_code = f.promo_code
)

SELECT *
FROM final
