MODEL (
  description """
    One row per campaign, day and platform metric, flattened from the daily spend export
  """,
  columns (
    spend_metric_key (type VARCHAR, nullable false, audits [not_null, unique]),
    spend_id (type VARCHAR, audits [not_null]),
    spend_date (type DATE, audits [not_null]),
    channel (type VARCHAR),
    campaign_id (type VARCHAR),
    currency_code (type VARCHAR),
    metric_name (type VARCHAR, audits [not_null]),
    metric_value (type NUMBER(38, 6)),
    platform_account_id (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__campaign_spend")
),

@sql.cdc_latest(src, src.key := (spend_id))

metrics AS (
  SELECT
    s.spend_id,
    s.spend_date,
    s.channel,
    s.campaign_id,
    s.currency_code,
    m.value:name::VARCHAR AS metric_name,
    m.value:value AS metric_raw,
    s.report:account.id::VARCHAR AS platform_account_id
  FROM cdc_latest AS s, LATERAL FLATTEN(input => s.report:metrics) AS m
), final AS (
  SELECT
    (
      m.spend_id || ':' || LOWER(m.metric_name)
    )::VARCHAR AS spend_metric_key,
    m.spend_id,
    m.spend_date,
    UPPER(m.channel)::VARCHAR AS channel,
    m.campaign_id,
    UPPER(m.currency_code)::VARCHAR AS currency_code,
    LOWER(REGEXP_REPLACE(m.metric_name, '([a-z])([A-Z])', '\\1_\\2'))::VARCHAR AS metric_name,
    CASE TYPEOF(m.metric_raw)
      WHEN 'VARCHAR'
      THEN TRY_TO_DECIMAL(REPLACE(
        m.metric_raw::VARCHAR,
        ',',
        ''
      ), 38, 6)
      ELSE m.metric_raw::NUMBER(38, 6)
    END::NUMBER(38, 6) AS metric_value,
    m.platform_account_id::VARCHAR AS platform_account_id
  FROM metrics AS m
)
SELECT *
FROM final
