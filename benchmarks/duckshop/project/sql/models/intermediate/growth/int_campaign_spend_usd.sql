MODEL (
  description """
    One row per ad platform spend report with its long-format metrics pivoted to columns, the spend
    converted to USD at the reference rate on the spend date, and whether the campaign was in
    flight.
  """,
  columns (
    spend_id (type VARCHAR, nullable false, audits [not_null, unique]),
    campaign_id (type VARCHAR),
    campaign_name (type VARCHAR),
    channel (type VARCHAR, audits [not_null]),
    platform_account_id (type VARCHAR),
    spend_date (type DATE, audits [not_null]),
    currency_code (type VARCHAR),
    spend_amount (type NUMBER(38, 6)),
    spend_usd (type NUMBER(18, 2)),
    impressions (type INT),
    clicks (type INT),
    conversions (type INT),
    click_through_rate (type NUMBER(9, 6)),
    cost_per_click_usd (type NUMBER(18, 4)),
    metric_count (type INT),
    unknown_metrics (type ARRAY),
    is_in_flight (type BOOLEAN),
  ),
);

WITH spend AS (
  SELECT *
  FROM __ref("stg_campaign_spend")
),

campaigns AS (
  SELECT *
  FROM __ref("stg_campaigns")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

report_headers AS (
  SELECT
    s.spend_id,
    s.spend_date,
    s.channel,
    s.campaign_id,
    s.currency_code,
    s.platform_account_id
  FROM spend AS s
  QUALIFY ROW_NUMBER() OVER (PARTITION BY s.spend_id ORDER BY s.spend_metric_key) = 1
),

metric_values AS (
  SELECT
    s.spend_id,
    MAX(IFF(s.metric_name = 'spend', s.metric_value, NULL)) AS spend_amount,
    MAX(IFF(s.metric_name = 'impressions', s.metric_value, NULL)) AS impressions,
    MAX(IFF(s.metric_name IN ('clicks', 'link_clicks'), s.metric_value, NULL)) AS clicks,
    MAX(
      IFF(REGEXP_LIKE(s.metric_name, '^(conversions|purchases)(_[0-9]+d)?$'), s.metric_value, NULL)
    ) AS conversions,
    COUNT(*) AS metric_count,
    ARRAY_AGG(
      DISTINCT IFF(
        REGEXP_LIKE(
          s.metric_name,
          '^(spend|impressions|clicks|link_clicks|conversions|purchases)(_[0-9]+d)?$'
        ),
        NULL,
        s.metric_name
      )
    ) WITHIN GROUP (
      ORDER BY IFF(
        REGEXP_LIKE(
          s.metric_name,
          '^(spend|impressions|clicks|link_clicks|conversions|purchases)(_[0-9]+d)?$'
        ),
        NULL,
        s.metric_name
      )
    ) AS unknown_metrics
  FROM spend AS s
  GROUP BY
    s.spend_id
),

reports AS (
  SELECT
    h.spend_id,
    h.spend_date,
    h.channel,
    h.campaign_id,
    h.currency_code,
    h.platform_account_id,
    m.spend_amount,
    m.impressions,
    m.clicks,
    m.conversions,
    m.metric_count,
    m.unknown_metrics
  FROM report_headers AS h
  INNER JOIN metric_values AS m
    ON h.spend_id = m.spend_id
),

with_rate AS (
  SELECT
    r.spend_id,
    r.spend_date,
    r.channel,
    r.campaign_id,
    r.currency_code,
    r.platform_account_id,
    r.spend_amount,
    r.impressions,
    r.clicks,
    r.conversions,
    r.metric_count,
    r.unknown_metrics,
    fx.rate_to_usd
  FROM reports AS r
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (r.spend_date >= fx.rate_date)
    ON r.currency_code = fx.currency_code
),

with_campaign AS (
  SELECT
    w.spend_id,
    w.campaign_id,
    c.campaign_name,
    COALESCE(w.channel, c.channel) AS channel,
    w.platform_account_id,
    w.spend_date,
    w.currency_code,
    w.spend_amount,
    w.spend_amount * w.rate_to_usd AS spend_usd_unrounded,
    w.impressions,
    w.clicks,
    w.conversions,
    w.metric_count,
    w.unknown_metrics,
    COALESCE(
      w.spend_date BETWEEN c.starts_on AND COALESCE(c.ends_on, '@@valuation_date'::DATE),
      FALSE
    ) AS is_in_flight
  FROM with_rate AS w
  LEFT JOIN campaigns AS c
    ON w.campaign_id = c.campaign_id
),

final AS (
  SELECT
    c.spend_id,
    c.campaign_id,
    c.campaign_name::VARCHAR AS campaign_name,
    c.channel::VARCHAR AS channel,
    c.platform_account_id,
    c.spend_date,
    c.currency_code,
    c.spend_amount::NUMBER(38, 6) AS spend_amount,
    @py.round_half_even("c.spend_usd_unrounded", 2)::NUMBER(18, 2) AS spend_usd,
    c.impressions::INT AS impressions,
    c.clicks::INT AS clicks,
    c.conversions::INT AS conversions,
    ROUND(c.clicks / NULLIF(c.impressions, 0), 6)::NUMBER(9, 6) AS click_through_rate,
    ROUND(c.spend_usd_unrounded / NULLIF(c.clicks, 0), 4)::NUMBER(18, 4) AS cost_per_click_usd,
    c.metric_count::INT AS metric_count,
    c.unknown_metrics::ARRAY AS unknown_metrics,
    c.is_in_flight::BOOLEAN AS is_in_flight
  FROM with_campaign AS c
)

SELECT *
FROM final
