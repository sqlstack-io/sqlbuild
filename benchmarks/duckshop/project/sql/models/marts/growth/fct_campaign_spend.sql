MODEL (
  description """
    One row per ad platform spend report with USD spend, delivery metrics and campaign flight status
  """,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    spend_id (type VARCHAR, nullable false, audits [not_null, unique]),
    campaign_id (type VARCHAR),
    channel (type VARCHAR, audits [not_null]),
    spend_date (type DATE, audits [not_null]),
    currency_code (type VARCHAR),
    spend_usd (type NUMBER(18, 2), audits [non_negative]),
    impressions (type INT),
    clicks (type INT),
    conversions (type INT),
    cost_per_click_usd (type NUMBER(18, 4)),
    is_in_flight (type BOOLEAN),
    has_unknown_metrics (type BOOLEAN),
  ),
);

WITH spend AS (
  SELECT *
  FROM __ref("int_campaign_spend_usd")
),

final AS (
  SELECT
    s.spend_id,
    s.campaign_id,
    s.channel,
    s.spend_date,
    s.currency_code,
    s.spend_usd,
    s.impressions,
    s.clicks,
    s.conversions,
    s.cost_per_click_usd,
    s.is_in_flight,
    (
      ARRAY_SIZE(ARRAY_COMPACT(s.unknown_metrics)) > 0
    )::BOOLEAN AS has_unknown_metrics
  FROM spend AS s
)

SELECT *
FROM final
