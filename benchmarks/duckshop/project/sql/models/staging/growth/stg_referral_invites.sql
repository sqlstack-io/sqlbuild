MODEL (
  description "One row per live referral invite",
  columns (
    invite_id (type VARCHAR, nullable false, audits [not_null, unique]),
    referrer_customer_id (type VARCHAR, audits [not_null]),
    invitee_email (type VARCHAR),
    sent_at (type TIMESTAMP_TZ),
    accepted_at (type TIMESTAMP_TZ),
    reward_amount_minor (type INT),
    reward_currency_code (type VARCHAR),
    reward (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__referral_invites")
),

@sql.cdc_latest(src, src.key := (invite_id))

final AS (
  SELECT
    i.invite_id,
    i.referrer_customer_id,
    @sql.normalise_email(i.invitee_email) AS invitee_email,
    i.sent_at,
    i.accepted_at,
    i.reward:amount_minor::INT AS reward_amount_minor,
    UPPER(i.reward:currency::VARCHAR)::VARCHAR AS reward_currency_code,
    i.reward::OBJECT AS reward,
    @sql.load_metadata(i)
  FROM cdc_latest AS i
)
SELECT *
FROM final
