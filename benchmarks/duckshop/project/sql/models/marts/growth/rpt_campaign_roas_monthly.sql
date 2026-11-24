MODEL (
  description """
    One row per month and channel with USD spend, attributed revenue from paid sessions and return
    on ad spend
  """,
  columns (
    roas_key (type VARCHAR, nullable false, audits [not_null, unique]),
    spend_month (type DATE),
    channel (type VARCHAR),
    spend_usd (type NUMBER(18, 2)),
    clicks (type INT),
    attributed_orders (type INT),
    attributed_revenue_usd (type NUMBER(18, 2)),
    return_on_ad_spend (type NUMBER(12, 4)),
  ),
);

WITH spend AS (
  SELECT *
  FROM __ref("fct_campaign_spend")
),

orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

campaigns AS (
  SELECT *
  FROM __ref("dim_campaigns")
),

campaign_keys AS (
  SELECT
    LOWER(c.campaign_id) AS utm_campaign,
    c.channel
  FROM campaigns AS c
),

monthly_spend AS (
  SELECT
    DATE_TRUNC('month', s.spend_date) AS spend_month,
    s.channel,
    SUM(s.spend_usd) AS spend_usd,
    SUM(s.clicks) AS clicks
  FROM spend AS s
  GROUP BY
    1,
    2
),

monthly_revenue AS (
  SELECT
    DATE_TRUNC('month', o.placed_date_local) AS spend_month,
    k.channel,
    COUNT(*) AS attributed_orders,
    SUM(o.amount_usd) AS attributed_revenue_usd
  FROM orders AS o
  INNER JOIN campaign_keys AS k
    ON o.utm_campaign = k.utm_campaign
  GROUP BY
    1,
    2
),

final AS (
  SELECT
    (
      TO_VARCHAR(s.spend_month, 'YYYY-MM') || ':' || s.channel
    )::VARCHAR AS roas_key,
    s.spend_month::DATE AS spend_month,
    s.channel,
    s.spend_usd::NUMBER(18, 2) AS spend_usd,
    s.clicks::INT AS clicks,
    COALESCE(r.attributed_orders, 0)::INT AS attributed_orders,
    COALESCE(r.attributed_revenue_usd, 0)::NUMBER(18, 2) AS attributed_revenue_usd,
    ROUND(r.attributed_revenue_usd / NULLIF(s.spend_usd, 0), 4)::NUMBER(12, 4) AS return_on_ad_spend
  FROM monthly_spend AS s
  LEFT JOIN monthly_revenue AS r
    ON s.spend_month = r.spend_month
    AND s.channel = r.channel
)

SELECT *
FROM final
