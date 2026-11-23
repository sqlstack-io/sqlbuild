MODEL (
  description """
    One row per marketing campaign with its flight dates, budget, targeting and lifetime results
  """,
  contract enforced,
  model_schema audit_ts,
  columns (
    campaign_id (type VARCHAR, nullable false, audits [not_null, unique]),
    campaign_name (type VARCHAR),
    channel (type VARCHAR),
    objective (type VARCHAR),
    starts_on (type DATE),
    ends_on (type DATE),
    flight_days (type INT),
    campaign_state (type VARCHAR),
    target_age_range (type VARCHAR),
    target_regions (type ARRAY),
    budget_usd (type NUMBER(18, 2)),
    spend_usd (type NUMBER(18, 2)),
    return_on_ad_spend (type NUMBER(12, 4)),
  ),
);

WITH campaigns AS (
  SELECT *
  FROM __ref("stg_campaigns")
),

performance AS (
  SELECT *
  FROM __ref("int_campaign_performance")
),

final AS (
  SELECT
    c.campaign_id,
    c.campaign_name,
    c.channel,
    c.objective,
    c.starts_on,
    c.ends_on,
    (
      DATEDIFF(
        'day',
        c.starts_on,
        COALESCE(c.ends_on, '@@valuation_date'::DATE)
      )
      + 1
    )::INT AS flight_days,
    CASE
      WHEN c.starts_on > '@@valuation_date'::DATE
      THEN 'scheduled'
      WHEN c.ends_on < '@@valuation_date'::DATE
      THEN 'ended'
      ELSE 'live'
    END::VARCHAR AS campaign_state,
    IFF(
      c.target_age_min IS NULL AND c.target_age_max IS NULL,
      'all',
      COALESCE(c.target_age_min::VARCHAR, '') || '-' || COALESCE(c.target_age_max::VARCHAR, '')
    )::VARCHAR AS target_age_range,
    c.target_regions,
    p.budget_usd,
    p.spend_usd,
    p.return_on_ad_spend,
    c.source_loaded_at
  FROM campaigns AS c
  LEFT JOIN performance AS p
    ON c.campaign_id = p.campaign_id
)

SELECT *
FROM final
