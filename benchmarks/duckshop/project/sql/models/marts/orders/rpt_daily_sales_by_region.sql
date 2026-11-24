MODEL (
  description """
    One row per local order date and region with orders, units, revenue in USD and average order
    value, plus week-over-week and trailing 28-day comparisons.
  """,
  columns (
    sales_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    placed_date_local (type DATE, audits [not_null]),
    region_code (type VARCHAR),
    order_count (type INT),
    customer_count (type INT),
    first_order_count (type INT),
    units (type INT),
    revenue_usd (type NUMBER(18, 2)),
    discount_usd (type NUMBER(18, 2)),
    average_order_usd (type NUMBER(18, 2)),
    revenue_same_day_last_week_usd (type NUMBER(18, 2)),
    revenue_trailing_28d_usd (type NUMBER(18, 2)),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

daily AS (
  SELECT
    o.placed_date_local,
    COALESCE(o.region_code, 'UNKNOWN') AS region_code,
    COUNT(*) AS order_count,
    COUNT(DISTINCT o.customer_id) AS customer_count,
    COUNT_IF(o.is_first_order) AS first_order_count,
    SUM(o.units) AS units,
    SUM(o.amount_usd) AS revenue_usd,
    SUM(o.discount_amount_usd) AS discount_usd
  FROM orders AS o
  WHERE o.order_status NOT IN ('CANCELLED', 'PENDING')
  GROUP BY
    o.placed_date_local,
    COALESCE(o.region_code, 'UNKNOWN')
),

final AS (
  SELECT
    (
      d.region_code || ':' || TO_VARCHAR(d.placed_date_local, 'YYYY-MM-DD')
    )::VARCHAR AS sales_day_key,
    d.placed_date_local,
    d.region_code::VARCHAR AS region_code,
    d.order_count::INT AS order_count,
    d.customer_count::INT AS customer_count,
    d.first_order_count::INT AS first_order_count,
    d.units::INT AS units,
    d.revenue_usd::NUMBER(18, 2) AS revenue_usd,
    d.discount_usd::NUMBER(18, 2) AS discount_usd,
    ROUND(d.revenue_usd / NULLIF(d.order_count, 0), 2)::NUMBER(18, 2) AS average_order_usd,
    SUM(d.revenue_usd) OVER (
      PARTITION BY d.region_code
      ORDER BY d.placed_date_local
      RANGE BETWEEN INTERVAL '7 days' PRECEDING AND INTERVAL '7 days' PRECEDING
    )::NUMBER(18, 2) AS revenue_same_day_last_week_usd,
    SUM(d.revenue_usd) OVER (
      PARTITION BY d.region_code
      ORDER BY d.placed_date_local
      RANGE BETWEEN INTERVAL '27 days' PRECEDING AND CURRENT ROW
    )::NUMBER(18, 2) AS revenue_trailing_28d_usd
  FROM daily AS d
)

SELECT *
FROM final
