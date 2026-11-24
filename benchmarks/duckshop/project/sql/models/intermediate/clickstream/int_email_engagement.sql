MODEL (
  description """
    One row per email message with its delivery outcome and human engagement, machine opens
    excluded, pivoted from the event stream, with time from send to first click.
  """,
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    campaign_id (type VARCHAR),
    recipient_email (type VARCHAR),
    sent_at (type TIMESTAMP_TZ),
    is_delivered (type BOOLEAN),
    bounce_type (type VARCHAR),
    human_open_count (type INT),
    click_count (type INT),
    clicked_domains (type ARRAY),
    unsubscribed (type BOOLEAN),
    complained (type BOOLEAN),
    minutes_to_first_click (type INT),
    customer_id (type VARCHAR),
    campaign_channel (type VARCHAR),
    is_campaign_in_flight (type BOOLEAN),
  ),
);

WITH email_events AS (
  SELECT *
  FROM __ref("stg_email_events")
),

customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

campaigns AS (
  SELECT *
  FROM __ref("stg_campaigns")
),

email_accounts AS (
  SELECT
    c.email_normalised,
    MIN(c.customer_id) AS customer_id
  FROM customers AS c
  WHERE c.email_normalised IS NOT NULL
  GROUP BY
    c.email_normalised
),

per_message AS (
  SELECT
    e.message_id,
    MAX(e.campaign_id) AS campaign_id,
    MAX(e.recipient_email) AS recipient_email,
    MIN(IFF(e.event_type = 'sent', e.occurred_at, NULL)) AS sent_at,
    BOOLOR_AGG(e.event_type = 'delivered') AS is_delivered,
    MAX(e.bounce_type) AS bounce_type,
    COUNT_IF(e.event_type = 'open' AND NOT COALESCE(e.is_machine_open, FALSE)) AS human_open_count,
    COUNT_IF(e.event_type = 'click') AS click_count,
    ARRAY_AGG(DISTINCT LOWER(PARSE_URL(e.clicked_url, 1)['host']::VARCHAR)) WITHIN GROUP (
      ORDER BY LOWER(PARSE_URL(e.clicked_url, 1)['host']::VARCHAR)
    ) AS clicked_domains,
    BOOLOR_AGG(e.event_type = 'unsubscribe') AS unsubscribed,
    BOOLOR_AGG(e.event_type = 'complaint') AS complained,
    MIN(IFF(e.event_type = 'click', e.occurred_at, NULL)) AS first_click_at
  FROM email_events AS e
  GROUP BY
    e.message_id
),

with_context AS (
  SELECT
    p.message_id,
    p.campaign_id,
    p.recipient_email,
    p.sent_at,
    p.is_delivered,
    p.bounce_type,
    p.human_open_count,
    p.click_count,
    p.clicked_domains,
    p.unsubscribed,
    p.complained,
    p.first_click_at,
    a.customer_id,
    c.channel AS campaign_channel,
    CONVERT_TIMEZONE('UTC', p.sent_at)::DATE BETWEEN c.starts_on AND COALESCE(
      c.ends_on,
      '@@valuation_date'::DATE
    ) AS is_campaign_in_flight
  FROM per_message AS p
  LEFT JOIN email_accounts AS a
    ON p.recipient_email = a.email_normalised
  LEFT JOIN campaigns AS c
    ON p.campaign_id = c.campaign_id
),

final AS (
  SELECT
    p.message_id,
    p.campaign_id::VARCHAR AS campaign_id,
    p.recipient_email::VARCHAR AS recipient_email,
    p.sent_at,
    COALESCE(p.is_delivered, FALSE)::BOOLEAN AS is_delivered,
    p.bounce_type::VARCHAR AS bounce_type,
    p.human_open_count::INT AS human_open_count,
    p.click_count::INT AS click_count,
    p.clicked_domains::ARRAY AS clicked_domains,
    COALESCE(p.unsubscribed, FALSE)::BOOLEAN AS unsubscribed,
    COALESCE(p.complained, FALSE)::BOOLEAN AS complained,
    DATEDIFF('minute', p.sent_at, p.first_click_at)::INT AS minutes_to_first_click,
    p.customer_id::VARCHAR AS customer_id,
    p.campaign_channel::VARCHAR AS campaign_channel,
    COALESCE(p.is_campaign_in_flight, FALSE)::BOOLEAN AS is_campaign_in_flight
  FROM with_context AS p
)

SELECT *
FROM final
