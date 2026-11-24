MODEL (
  description """
    One row per month, region and scorecard measure, unpivoted from a wide monthly summary of sales,
    retention and fulfilment so dashboards can chart any measure the same way.
  """,
  columns (
    scorecard_key (type VARCHAR, nullable false, audits [not_null, unique]),
    scorecard_month (type DATE),
    region_code (type VARCHAR),
    measure (type VARCHAR),
    measure_value (type NUMBER(18, 4)),
    rank_among_regions (type INT),
  ),
);

WITH sales AS (
  SELECT *
  FROM __ref("rpt_daily_sales_by_region")
),

recognition AS (
  SELECT *
  FROM __ref("rpt_revenue_recognition_monthly")
),

monthly_sales AS (
  SELECT
    DATE_TRUNC('month', s.placed_date_local)::DATE AS scorecard_month,
    s.region_code,
    SUM(s.order_count)::NUMBER(18, 4) AS orders,
    SUM(s.revenue_usd)::NUMBER(18, 4) AS revenue_usd,
    SUM(s.first_order_count)::NUMBER(18, 4) AS first_orders,
    (
      SUM(s.revenue_usd) / NULLIF(SUM(s.order_count), 0)
    )::NUMBER(18, 4) AS average_order_usd
  FROM sales AS s
  GROUP BY
    1,
    2
),

wide AS (
  SELECT
    m.scorecard_month,
    m.region_code,
    m.orders,
    m.revenue_usd,
    m.first_orders,
    m.average_order_usd,
    COALESCE(r.refund_ratio, 0)::NUMBER(18, 4) AS refund_ratio,
    COALESCE(r.subscription_usd, 0)::NUMBER(18, 4) AS subscription_usd
  FROM monthly_sales AS m
  LEFT JOIN recognition AS r
    ON m.scorecard_month = r.recognition_month
    AND m.region_code = r.region_code
),

long_form AS (
  SELECT
    u.scorecard_month,
    u.region_code,
    u.measure,
    u.measure_value
  FROM wide UNPIVOT(
    measure_value FOR measure IN (
      orders,
      revenue_usd,
      first_orders,
      average_order_usd,
      refund_ratio,
      subscription_usd
    )
  ) AS u
),

final AS (
  SELECT
    (
      TO_VARCHAR(l.scorecard_month, 'YYYY-MM') || ':' || l.region_code || ':' || LOWER(l.measure)
    )::VARCHAR AS scorecard_key,
    l.scorecard_month::DATE AS scorecard_month,
    l.region_code::VARCHAR AS region_code,
    LOWER(l.measure)::VARCHAR AS measure,
    l.measure_value::NUMBER(18, 4) AS measure_value,
    RANK() OVER (
      PARTITION BY l.scorecard_month, l.measure
      ORDER BY l.measure_value DESC
    )::INT AS rank_among_regions
  FROM long_form AS l
)

SELECT *
FROM final
