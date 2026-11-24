MODEL (
  description """
    One row per order with its position in the customer's order history, the gap since the previous
    order, and running lifetime totals in USD.
  """,
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    placed_at (type TIMESTAMP_TZ),
    placed_date_local (type DATE),
    order_number (type INT),
    is_first_order (type BOOLEAN),
    days_since_previous_order (type INT),
    amount_usd (type NUMBER(18, 2)),
    lifetime_revenue_usd (type NUMBER(18, 2)),
    rolling_90d_revenue_usd (type NUMBER(18, 2)),
    previous_sales_channel (type VARCHAR),
    is_channel_switch (type BOOLEAN),
    cohort_month (type DATE),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

valid_orders AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.placed_at,
    o.placed_date_local,
    o.amount_usd,
    o.sales_channel
  FROM orders AS o
  WHERE NOT @sql.is_status(@enum("order_status").CANCELLED)
),

sequenced AS (
  SELECT
    v.order_id,
    v.customer_id,
    v.placed_at,
    v.placed_date_local,
    v.amount_usd,
    v.sales_channel,
    ROW_NUMBER() OVER (PARTITION BY v.customer_id ORDER BY v.placed_at, v.order_id) AS order_number,
    LAG(v.placed_date_local) OVER (
      PARTITION BY v.customer_id
      ORDER BY v.placed_at, v.order_id
    ) AS previous_date,
    LAG(v.sales_channel) OVER (
      PARTITION BY v.customer_id
      ORDER BY v.placed_at, v.order_id
    ) AS previous_sales_channel,
    SUM(v.amount_usd) OVER (
      PARTITION BY v.customer_id
      ORDER BY v.placed_at, v.order_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS lifetime_revenue_usd,
    SUM(v.amount_usd) OVER (
      PARTITION BY v.customer_id
      ORDER BY v.placed_date_local
      RANGE BETWEEN INTERVAL '89 days' PRECEDING AND CURRENT ROW
    ) AS rolling_90d_revenue_usd,
    MIN(v.placed_date_local) OVER (PARTITION BY v.customer_id) AS first_order_date
  FROM valid_orders AS v
),

final AS (
  SELECT
    s.order_id,
    s.customer_id,
    s.placed_at,
    s.placed_date_local,
    s.order_number::INT AS order_number,
    (
      s.order_number = 1
    ) AS is_first_order,
    DATEDIFF('day', s.previous_date, s.placed_date_local)::INT AS days_since_previous_order,
    s.amount_usd,
    s.lifetime_revenue_usd::NUMBER(18, 2) AS lifetime_revenue_usd,
    s.rolling_90d_revenue_usd::NUMBER(18, 2) AS rolling_90d_revenue_usd,
    s.previous_sales_channel::VARCHAR AS previous_sales_channel,
    COALESCE(s.previous_sales_channel <> s.sales_channel, FALSE) AS is_channel_switch,
    DATE_TRUNC('month', s.first_order_date)::DATE AS cohort_month
  FROM sequenced AS s
)

SELECT *
FROM final
