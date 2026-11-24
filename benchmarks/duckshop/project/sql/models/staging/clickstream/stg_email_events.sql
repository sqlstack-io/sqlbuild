MODEL (
  description "One row per email delivery or engagement event",
  columns (
    email_event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    message_id (type VARCHAR, audits [not_null]),
    event_type (type VARCHAR, audits [
      accepted_values (
        values ["sent", "delivered", "open", "click", "bounce", "unsubscribe", "complaint"],
      ),
    ]),
    recipient_email (type VARCHAR),
    campaign_id (type VARCHAR),
    occurred_at (type TIMESTAMP_TZ, audits [not_null]),
    clicked_url (type VARCHAR),
    bounce_type (type VARCHAR),
    is_machine_open (type BOOLEAN),
    details (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("clickstream__email_events")
),

latest AS (
  SELECT
    e.email_event_id,
    e.message_id,
    e.event_type,
    e.recipient_email,
    e.campaign_id,
    e.occurred_at,
    e.details
  FROM src AS e
  QUALIFY ROW_NUMBER() OVER (PARTITION BY e.email_event_id ORDER BY e._loaded_at DESC) = 1
),

final AS (
  SELECT
    l.email_event_id,
    l.message_id,
    LOWER(l.event_type)::VARCHAR AS event_type,
    @sql.normalise_email(l.recipient_email) AS recipient_email,
    l.campaign_id,
    l.occurred_at,
    l.details:url::VARCHAR AS clicked_url,
    LOWER(l.details:bounce.type::VARCHAR)::VARCHAR AS bounce_type,
    COALESCE(l.details:machine_open::BOOLEAN, REGEXP_LIKE(
      l.details:user_agent::VARCHAR,
      '.*(Proxy|Prefetch).*'
    )) AS is_machine_open,
    l.details::OBJECT AS details
  FROM latest AS l
)

SELECT *
FROM final
