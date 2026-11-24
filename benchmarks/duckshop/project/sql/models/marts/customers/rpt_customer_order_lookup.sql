MODEL (
  description """
    One row per order of the customer named by the lookup_customer_id variable, read through the
    customer_orders table function so the customer filter is applied inside the function, with the
    customer's profile alongside. generated_at records when the lookup ran as a TIMESTAMP_LTZ from
    CURRENT_TIMESTAMP(), so it depends on the session time zone and the run, an expected divergence.
  """,
  tags [known_divergence],
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR),
    rfm_segment (type VARCHAR),
    loyalty_tier_code (type VARCHAR),
    placed_at (type TIMESTAMP_TZ),
    order_status (type VARCHAR),
    amount_usd (type NUMBER(18, 2)),
    order_number (type INT),
    running_total_usd (type NUMBER(18, 2)),
    share_of_lifetime_revenue (type NUMBER(9, 6)),
    generated_at (type TIMESTAMP_LTZ),
  ),
);

WITH customer_orders AS (
  SELECT *
  FROM __table_fn("table_fn__customer_orders")('@@lookup_customer_id')
),

customers AS (
  SELECT *
  FROM __ref("dim_customers")
),

keyed_orders AS (
  SELECT
    '@@lookup_customer_id'::VARCHAR AS customer_id,
    o.order_id,
    o.placed_at,
    o.order_status,
    o.amount_usd,
    o.order_number
  FROM customer_orders AS o
),

profile AS (
  SELECT
    c.customer_id,
    c.rfm_segment,
    c.loyalty_tier_code,
    c.lifetime_revenue_usd
  FROM customers AS c
  WHERE c.customer_id = '@@lookup_customer_id'
),

final AS (
  SELECT
    o.order_id::VARCHAR AS order_id,
    o.customer_id,
    p.rfm_segment,
    p.loyalty_tier_code,
    o.placed_at::TIMESTAMP_TZ AS placed_at,
    o.order_status::VARCHAR AS order_status,
    o.amount_usd::NUMBER(18, 2) AS amount_usd,
    o.order_number::INT AS order_number,
    SUM(o.amount_usd) OVER (
      ORDER BY o.order_number
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::NUMBER(18, 2) AS running_total_usd,
    ROUND(
      o.amount_usd / NULLIF(p.lifetime_revenue_usd, 0),
      6
    )::NUMBER(9, 6) AS share_of_lifetime_revenue,
    CURRENT_TIMESTAMP()::TIMESTAMP_LTZ AS generated_at
  FROM keyed_orders AS o
  LEFT JOIN profile AS p
    ON o.customer_id = p.customer_id
)

SELECT *
FROM final
