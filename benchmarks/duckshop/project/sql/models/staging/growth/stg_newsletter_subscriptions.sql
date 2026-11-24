MODEL (
  description "One row per live newsletter list membership, including explicit unsubscribes",
  columns (
    list_subscription_id (type VARCHAR, nullable false, audits [not_null, unique]),
    email (type VARCHAR),
    list_code (type VARCHAR),
    list_status (type VARCHAR, audits [
      accepted_values (values ["subscribed", "unsubscribed", "pending", "cleaned"]),
    ]),
    subscribed_at (type TIMESTAMP_TZ),
    unsubscribed_at (type TIMESTAMP_TZ),
    signup_form (type VARCHAR),
    is_double_opt_in (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__newsletter_subscriptions")
),

@sql.cdc_latest(src, src.key := (list_subscription_id))

final AS (
  SELECT
    s.list_subscription_id,
    @sql.normalise_email(s.email) AS email,
    UPPER(s.list_code)::VARCHAR AS list_code,
    LOWER(s.list_status)::VARCHAR AS list_status,
    s.subscribed_at,
    s.unsubscribed_at,
    @sql.json_text(s.signup_source:form) AS signup_form,
    COALESCE(s.signup_source:double_opt_in::BOOLEAN, FALSE) AS is_double_opt_in,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
)
SELECT *
FROM final
