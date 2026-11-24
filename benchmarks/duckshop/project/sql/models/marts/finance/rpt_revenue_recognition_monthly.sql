MODEL (
  description """
    One row per month and region with recognised product, refund and subscription revenue in USD
  """,
  columns (
    recognition_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    recognition_month (type DATE),
    region_code (type VARCHAR),
    product_usd (type NUMBER(18, 2)),
    refund_usd (type NUMBER(18, 2)),
    subscription_usd (type NUMBER(18, 2)),
    net_revenue_usd (type NUMBER(18, 2)),
    refund_ratio (type NUMBER(9, 4)),
  ),
);

WITH recognition AS (
  SELECT *
  FROM __ref("int_revenue_recognition_daily")
),

monthly AS (
  SELECT
    DATE_TRUNC('month', r.recognition_date) AS recognition_month,
    r.region_code,
    r.revenue_stream,
    r.revenue_usd
  FROM recognition AS r
),

pivoted AS (
  SELECT *
  FROM monthly PIVOT(
    SUM(revenue_usd) FOR revenue_stream IN ('product', 'refund', 'subscription')
  ) AS p(recognition_month, region_code, product_usd, refund_usd, subscription_usd)
),

final AS (
  SELECT
    (
      TO_VARCHAR(p.recognition_month, 'YYYY-MM') || ':' || p.region_code
    )::VARCHAR AS recognition_month_key,
    p.recognition_month::DATE AS recognition_month,
    p.region_code::VARCHAR AS region_code,
    COALESCE(p.product_usd, 0)::NUMBER(18, 2) AS product_usd,
    COALESCE(p.refund_usd, 0)::NUMBER(18, 2) AS refund_usd,
    COALESCE(p.subscription_usd, 0)::NUMBER(18, 2) AS subscription_usd,
    (
      COALESCE(p.product_usd, 0) + COALESCE(p.refund_usd, 0) + COALESCE(p.subscription_usd, 0)
    )::NUMBER(18, 2) AS net_revenue_usd,
    ROUND(-p.refund_usd / NULLIF(p.product_usd, 0), 4)::NUMBER(9, 4) AS refund_ratio
  FROM pivoted AS p
)

SELECT *
FROM final
