MODEL (
  description """
    One row per referral invite with the invitee matched to a customer account by normalised email,
    the invitee's first order, whether it landed inside the qualifying window, and the reward value
    in USD.
  """,
  constants (
    _qualifying_window_days 30,
    _minimum_qualifying_order_usd 20.00,
  ),
  columns (
    invite_id (type VARCHAR, nullable false, audits [not_null, unique]),
    referrer_customer_id (type VARCHAR, audits [not_null]),
    invitee_customer_id (type VARCHAR),
    invitee_email_domain (type VARCHAR),
    sent_at (type TIMESTAMP_TZ),
    accepted_at (type TIMESTAMP_TZ),
    hours_to_accept (type NUMBER(10, 2)),
    first_order_id (type VARCHAR),
    first_order_usd (type NUMBER(18, 2)),
    days_to_first_order (type INT),
    is_self_referral (type BOOLEAN),
    is_qualified (type BOOLEAN),
    reward_usd (type NUMBER(18, 2)),
    invite_sequence_for_referrer (type INT),
    referrer_lifetime_orders (type INT),
  ),
);

WITH invites AS (
  SELECT *
  FROM __ref("stg_referral_invites")
),

identities AS (
  SELECT *
  FROM __ref("int_customer_identity")
),

order_sequence AS (
  SELECT *
  FROM __ref("int_customer_order_sequence")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

sequenced_invites AS (
  SELECT
    i.invite_id,
    i.referrer_customer_id,
    i.invitee_email,
    REGEXP_SUBSTR(i.invitee_email, '@(.+)$', 1, 1, 'e', 1) AS invitee_email_domain,
    i.sent_at,
    i.accepted_at,
    CONVERT_TIMEZONE('UTC', i.sent_at)::DATE AS sent_date,
    i.reward_amount_minor,
    i.reward_currency_code,
    ROW_NUMBER() OVER (
      PARTITION BY i.referrer_customer_id
      ORDER BY i.sent_at, i.invite_id
    ) AS invite_sequence_for_referrer
  FROM invites AS i
),

email_accounts AS (
  SELECT
    d.email_normalised,
    MIN_BY(d.customer_id, d.customer_id) AS customer_id
  FROM identities AS d
  WHERE d.email_normalised IS NOT NULL
  GROUP BY
    d.email_normalised
),

first_orders AS (
  SELECT
    s.customer_id,
    MIN_BY(s.order_id, s.placed_at) AS first_order_id,
    MIN(s.placed_at) AS first_order_at,
    MIN_BY(s.amount_usd, s.placed_at) AS first_order_usd
  FROM order_sequence AS s
  GROUP BY
    s.customer_id
),

referrer_orders AS (
  SELECT
    s.customer_id,
    COUNT(*) AS lifetime_orders
  FROM order_sequence AS s
  GROUP BY
    s.customer_id
),

reward_rates AS (
  SELECT
    v.invite_id,
    @sql.money_from_minor(v.reward_amount_minor, cur.minor_unit_exponent, places := 6) * fx.rate_to_usd AS reward_usd
  FROM sequenced_invites AS v
  INNER JOIN currencies AS cur
    ON v.reward_currency_code = cur.currency_code
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (v.sent_date >= fx.rate_date)
    ON v.reward_currency_code = fx.currency_code
),

matched AS (
  SELECT
    v.invite_id,
    v.referrer_customer_id,
    a.customer_id AS invitee_customer_id,
    v.invitee_email_domain,
    v.sent_at,
    v.accepted_at,
    v.invite_sequence_for_referrer,
    f.first_order_id,
    f.first_order_at,
    f.first_order_usd,
    r.reward_usd,
    COALESCE(ro.lifetime_orders, 0) AS referrer_lifetime_orders
  FROM sequenced_invites AS v
  LEFT JOIN email_accounts AS a
    ON v.invitee_email = a.email_normalised
  LEFT JOIN first_orders AS f
    ON a.customer_id = f.customer_id
  LEFT JOIN reward_rates AS r
    ON v.invite_id = r.invite_id
  LEFT JOIN referrer_orders AS ro
    ON v.referrer_customer_id = ro.customer_id
),

final AS (
  SELECT
    m.invite_id,
    m.referrer_customer_id,
    m.invitee_customer_id::VARCHAR AS invitee_customer_id,
    m.invitee_email_domain::VARCHAR AS invitee_email_domain,
    m.sent_at,
    m.accepted_at,
    ROUND(DATEDIFF('minute', m.sent_at, m.accepted_at) / 60, 2)::NUMBER(10, 2) AS hours_to_accept,
    m.first_order_id::VARCHAR AS first_order_id,
    m.first_order_usd::NUMBER(18, 2) AS first_order_usd,
    DATEDIFF('day', m.sent_at, m.first_order_at)::INT AS days_to_first_order,
    EQUAL_NULL(m.invitee_customer_id, m.referrer_customer_id)::BOOLEAN AS is_self_referral,
    COALESCE(
      m.first_order_at >= m.sent_at
      AND DATEDIFF('day', m.sent_at, m.first_order_at) <= @const("_qualifying_window_days")
      AND m.first_order_usd >= @const("_minimum_qualifying_order_usd")
      AND NOT EQUAL_NULL(m.invitee_customer_id, m.referrer_customer_id),
      FALSE
    )::BOOLEAN AS is_qualified,
    ROUND(m.reward_usd, 2)::NUMBER(18, 2) AS reward_usd,
    m.invite_sequence_for_referrer::INT AS invite_sequence_for_referrer,
    m.referrer_lifetime_orders::INT AS referrer_lifetime_orders
  FROM matched AS m
)

SELECT *
FROM final
