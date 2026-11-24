MODEL (
  description "One row per region and RFM segment with customer counts, revenue and VIP share",
  columns (
    segment_key (type VARCHAR, nullable false, audits [not_null, unique]),
    region_code (type VARCHAR),
    rfm_segment (type VARCHAR),
    customers (type INT),
    vip_customers (type INT),
    lifetime_revenue_usd (type NUMBER(18, 2)),
    revenue_share_in_region (type NUMBER(9, 4)),
    median_days_since_last_order (type NUMBER(10, 2)),
  ),
);

WITH customers AS (
  SELECT *
  FROM __ref("dim_customers")
),

grouped AS (
  SELECT
    COALESCE(c.region_code, 'UNKNOWN') AS region_code,
    c.rfm_segment,
    COUNT(*) AS customers,
    COUNT_IF(c.is_vip) AS vip_customers,
    SUM(c.lifetime_revenue_usd) AS lifetime_revenue_usd,
    MEDIAN(c.days_since_last_order) AS median_days_since_last_order
  FROM customers AS c
  GROUP BY
    1,
    2
),

final AS (
  SELECT
    (
      g.region_code || ':' || g.rfm_segment
    )::VARCHAR AS segment_key,
    g.region_code::VARCHAR AS region_code,
    g.rfm_segment,
    g.customers::INT AS customers,
    g.vip_customers::INT AS vip_customers,
    g.lifetime_revenue_usd::NUMBER(18, 2) AS lifetime_revenue_usd,
    ROUND(
      RATIO_TO_REPORT(g.lifetime_revenue_usd) OVER (PARTITION BY g.region_code),
      4
    )::NUMBER(9, 4) AS revenue_share_in_region,
    ROUND(g.median_days_since_last_order, 2)::NUMBER(10, 2) AS median_days_since_last_order
  FROM grouped AS g
)

SELECT *
FROM final
