MODEL (
  description """
    One row per promotion with redemption counts, discounted revenue in USD, the share of orders
    that met the best tier, and incremental basket size against non-promotion orders. Whether a
    promotion is active now is read from CURRENT_TIMESTAMP(), which is session-dependent and an
    expected divergence between runs and engines.
  """,
  tags [known_divergence],
  columns (
    promo_code (type VARCHAR, nullable false, audits [not_null, unique]),
    promo_name (type VARCHAR),
    is_active_now (type BOOLEAN),
    redemption_count (type INT),
    redeeming_customers (type INT),
    revenue_usd (type NUMBER(18, 2)),
    discount_usd (type NUMBER(18, 2)),
    discount_rate (type NUMBER(9, 6)),
    avg_order_usd (type NUMBER(18, 2)),
    baseline_avg_order_usd (type NUMBER(18, 2)),
    basket_uplift_pct (type NUMBER(9, 2)),
    top_tier_threshold_minor (type INT),
    eligible_region_count (type INT),
  ),
);

WITH promotions AS (
  SELECT *
  FROM __ref("stg_promotions")
),

redemptions AS (
  SELECT *
  FROM __ref("stg_promotion_redemptions")
),

orders_usd AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

tiers AS (
  SELECT
    p.promo_code,
    t.value:threshold_minor::INT AS threshold_minor
  FROM promotions AS p, LATERAL FLATTEN(input => p.rules:tiers, outer => TRUE) AS t
),

top_tiers AS (
  SELECT
    t.promo_code,
    MAX(t.threshold_minor) AS top_tier_threshold_minor
  FROM tiers AS t
  GROUP BY
    t.promo_code
),

redeemed_orders AS (
  SELECT
    r.promo_code,
    r.order_id,
    o.customer_id,
    o.amount_usd,
    o.discount_amount_usd
  FROM redemptions AS r
  INNER JOIN orders_usd AS o
    ON r.order_id = o.order_id
),

promo_totals AS (
  SELECT
    ro.promo_code,
    COUNT(DISTINCT ro.order_id) AS redemption_count,
    COUNT(DISTINCT ro.customer_id) AS redeeming_customers,
    SUM(ro.amount_usd) AS revenue_usd,
    SUM(ro.discount_amount_usd) AS discount_usd,
    AVG(ro.amount_usd) AS avg_order_usd
  FROM redeemed_orders AS ro
  GROUP BY
    ro.promo_code
),

baseline AS (
  SELECT AVG(o.amount_usd) AS baseline_avg_order_usd
  FROM orders_usd AS o
  WHERE o.promo_code IS NULL
),

final AS (
  SELECT
    p.promo_code,
    p.promo_name,
    (
      CURRENT_TIMESTAMP() BETWEEN p.starts_at AND COALESCE(p.ends_at, '9999-12-31'::TIMESTAMP_TZ)
    ) AS is_active_now,
    COALESCE(pt.redemption_count, 0)::INT AS redemption_count,
    COALESCE(pt.redeeming_customers, 0)::INT AS redeeming_customers,
    COALESCE(pt.revenue_usd, 0)::NUMBER(18, 2) AS revenue_usd,
    COALESCE(pt.discount_usd, 0)::NUMBER(18, 2) AS discount_usd,
    @sql.safe_divide(pt.discount_usd, pt.revenue_usd + pt.discount_usd)::NUMBER(9, 6) AS discount_rate,
    ROUND(pt.avg_order_usd, 2)::NUMBER(18, 2) AS avg_order_usd,
    ROUND(b.baseline_avg_order_usd, 2)::NUMBER(18, 2) AS baseline_avg_order_usd,
    ROUND(100
    * (
      pt.avg_order_usd / NULLIF(b.baseline_avg_order_usd, 0) - 1
    ), 2)::NUMBER(9, 2) AS basket_uplift_pct,
    tt.top_tier_threshold_minor::INT AS top_tier_threshold_minor,
    ARRAY_SIZE(p.eligible_regions)::INT AS eligible_region_count
  FROM promotions AS p
  LEFT JOIN promo_totals AS pt
    ON p.promo_code = pt.promo_code
  LEFT JOIN top_tiers AS tt
    ON p.promo_code = tt.promo_code
  CROSS JOIN baseline AS b
)

SELECT *
FROM final
