MODEL (
  description """
    One row per invite month with invites sent, acceptance, qualified referrals and reward cost
  """,
  columns (
    invite_month (type DATE, nullable false, audits [not_null, unique]),
    invites_sent (type INT),
    accepted (type INT),
    qualified (type INT),
    self_referrals (type INT),
    acceptance_rate (type NUMBER(9, 4)),
    reward_cost_usd (type NUMBER(18, 2)),
    referred_revenue_usd (type NUMBER(18, 2)),
    cost_per_qualified_usd (type NUMBER(18, 2)),
  ),
);

WITH referrals AS (
  SELECT *
  FROM __ref("int_referral_conversions")
),

monthly AS (
  SELECT
    DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', r.sent_at))::DATE AS invite_month,
    COUNT(*) AS invites_sent,
    COUNT(r.accepted_at) AS accepted,
    COUNT_IF(r.is_qualified) AS qualified,
    COUNT_IF(r.is_self_referral) AS self_referrals,
    SUM(IFF(r.is_qualified, r.reward_usd, 0)) AS reward_cost_usd,
    SUM(IFF(r.is_qualified, r.first_order_usd, 0)) AS referred_revenue_usd
  FROM referrals AS r
  GROUP BY
    1
),

final AS (
  SELECT
    m.invite_month,
    m.invites_sent::INT AS invites_sent,
    m.accepted::INT AS accepted,
    m.qualified::INT AS qualified,
    m.self_referrals::INT AS self_referrals,
    ROUND(m.accepted / NULLIF(m.invites_sent, 0), 4)::NUMBER(9, 4) AS acceptance_rate,
    m.reward_cost_usd::NUMBER(18, 2) AS reward_cost_usd,
    m.referred_revenue_usd::NUMBER(18, 2) AS referred_revenue_usd,
    ROUND(m.reward_cost_usd / NULLIF(m.qualified, 0), 2)::NUMBER(18, 2) AS cost_per_qualified_usd
  FROM monthly AS m
)

SELECT *
FROM final
