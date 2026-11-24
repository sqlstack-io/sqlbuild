MODEL (
  description """
    One row per campaign with USD spend, paid and email touches, attributed sessions and orders, and
    return on ad spend. Orders and sessions are attributed when their utm_campaign matches the
    campaign identifier.
  """,
  columns (
    campaign_id (type VARCHAR, nullable false, audits [not_null, unique]),
    campaign_name (type VARCHAR),
    channel (type VARCHAR),
    objective (type VARCHAR),
    starts_on (type DATE),
    ends_on (type DATE),
    budget_usd (type NUMBER(18, 2)),
    spend_usd (type NUMBER(18, 2)),
    budget_used_pct (type NUMBER(9, 2)),
    paid_clicks (type INT),
    email_clicks (type INT),
    attributed_sessions (type INT),
    attributed_orders (type INT),
    attributed_revenue_usd (type NUMBER(18, 2)),
    return_on_ad_spend (type NUMBER(12, 4)),
    target_region_count (type INT),
    unsupported_target_regions (type ARRAY),
    spend_rank_in_channel (type INT),
  ),
);

WITH campaigns AS (
  SELECT *
  FROM __ref("stg_campaigns")
),

spend AS (
  SELECT *
  FROM __ref("int_campaign_spend_usd")
),

sessions AS (
  SELECT *
  FROM __ref("int_session_attribution")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

emails AS (
  SELECT *
  FROM __ref("int_email_engagement")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

campaign_keys AS (
  SELECT
    c.campaign_id,
    LOWER(c.campaign_id) AS utm_campaign_key,
    c.campaign_name,
    c.channel,
    c.objective,
    c.starts_on,
    c.ends_on,
    c.budget_minor,
    c.currency_code,
    c.target_regions
  FROM campaigns AS c
),

budget_rate AS (
  SELECT
    k.campaign_id,
    ROUND(
      @sql.money_from_minor(k.budget_minor, cur.minor_unit_exponent, places := 6) * fx.rate_to_usd,
      2
    ) AS budget_usd
  FROM campaign_keys AS k
  INNER JOIN currencies AS cur
    ON k.currency_code = cur.currency_code
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (k.starts_on >= fx.rate_date)
    ON k.currency_code = fx.currency_code
),

target_regions AS (
  SELECT
    k.campaign_id,
    COUNT(r.value) AS target_region_count,
    ARRAY_AGG(
      IFF(@sql.in_regions(r.value::VARCHAR, @const("supported_regions")), NULL, r.value::VARCHAR)
    ) WITHIN GROUP (ORDER BY r.value::VARCHAR) AS unsupported_target_regions
  FROM campaign_keys AS k, LATERAL FLATTEN(input => k.target_regions) AS r
  GROUP BY
    k.campaign_id
),

touches AS (
  SELECT
    s.campaign_id::VARCHAR AS campaign_id,
    'paid'::VARCHAR AS touch_type,
    COALESCE(s.clicks, 0)::INT AS touch_count
  FROM spend AS s
  WHERE s.campaign_id IS NOT NULL
  UNION ALL
  SELECT
    e.campaign_id::VARCHAR AS campaign_id,
    'email'::VARCHAR AS touch_type,
    e.click_count::INT AS touch_count
  FROM emails AS e
  WHERE
    e.campaign_id IS NOT NULL
    AND e.click_count > 0
),

touch_totals AS (
  SELECT
    t.campaign_id,
    SUM(IFF(t.touch_type = 'paid', t.touch_count, 0)) AS paid_clicks,
    SUM(IFF(t.touch_type = 'email', t.touch_count, 0)) AS email_clicks
  FROM touches AS t
  GROUP BY
    t.campaign_id
),

spend_totals AS (
  SELECT
    s.campaign_id,
    SUM(s.spend_usd) AS spend_usd
  FROM spend AS s
  GROUP BY
    s.campaign_id
),

session_totals AS (
  SELECT
    s.utm_campaign,
    COUNT(*) AS attributed_sessions
  FROM sessions AS s
  WHERE s.utm_campaign IS NOT NULL
  GROUP BY
    s.utm_campaign
),

order_totals AS (
  SELECT
    o.utm_campaign,
    COUNT(*) AS attributed_orders,
    SUM(o.amount_usd) AS attributed_revenue_usd
  FROM orders AS o
  WHERE
    o.utm_campaign IS NOT NULL
    AND o.order_status NOT IN ('CANCELLED', 'PENDING')
  GROUP BY
    o.utm_campaign
),

combined AS (
  SELECT
    k.campaign_id,
    k.campaign_name,
    k.channel,
    k.objective,
    k.starts_on,
    k.ends_on,
    b.budget_usd,
    COALESCE(sp.spend_usd, 0) AS spend_usd,
    COALESCE(t.paid_clicks, 0) AS paid_clicks,
    COALESCE(t.email_clicks, 0) AS email_clicks,
    COALESCE(st.attributed_sessions, 0) AS attributed_sessions,
    COALESCE(ot.attributed_orders, 0) AS attributed_orders,
    COALESCE(ot.attributed_revenue_usd, 0) AS attributed_revenue_usd,
    COALESCE(tr.target_region_count, 0) AS target_region_count,
    COALESCE(tr.unsupported_target_regions, ARRAY_CONSTRUCT()) AS unsupported_target_regions
  FROM campaign_keys AS k
  LEFT JOIN budget_rate AS b
    ON k.campaign_id = b.campaign_id
  LEFT JOIN spend_totals AS sp
    ON k.campaign_id = sp.campaign_id
  LEFT JOIN touch_totals AS t
    ON k.campaign_id = t.campaign_id
  LEFT JOIN session_totals AS st
    ON k.utm_campaign_key = st.utm_campaign
  LEFT JOIN order_totals AS ot
    ON k.utm_campaign_key = ot.utm_campaign
  LEFT JOIN target_regions AS tr
    ON k.campaign_id = tr.campaign_id
),

final AS (
  SELECT
    c.campaign_id,
    c.campaign_name,
    c.channel,
    c.objective,
    c.starts_on,
    c.ends_on,
    c.budget_usd::NUMBER(18, 2) AS budget_usd,
    c.spend_usd::NUMBER(18, 2) AS spend_usd,
    ROUND(100 * c.spend_usd / NULLIF(c.budget_usd, 0), 2)::NUMBER(9, 2) AS budget_used_pct,
    c.paid_clicks::INT AS paid_clicks,
    c.email_clicks::INT AS email_clicks,
    c.attributed_sessions::INT AS attributed_sessions,
    c.attributed_orders::INT AS attributed_orders,
    c.attributed_revenue_usd::NUMBER(18, 2) AS attributed_revenue_usd,
    ROUND(
      c.attributed_revenue_usd / NULLIF(c.spend_usd, 0),
      4
    )::NUMBER(12, 4) AS return_on_ad_spend,
    c.target_region_count::INT AS target_region_count,
    c.unsupported_target_regions::ARRAY AS unsupported_target_regions,
    DENSE_RANK() OVER (
      PARTITION BY c.channel
      ORDER BY c.spend_usd DESC
    )::INT AS spend_rank_in_channel
  FROM combined AS c
)

SELECT *
FROM final
