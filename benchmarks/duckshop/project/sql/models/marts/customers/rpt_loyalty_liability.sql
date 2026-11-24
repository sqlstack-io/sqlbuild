MODEL (
  description """
    One row per loyalty tier with outstanding points, their USD liability at the valuation date, and
    points due to expire in the next 90 days.
  """,
  columns (
    tier_code (type VARCHAR, nullable false, audits [not_null, unique]),
    member_count (type INT),
    outstanding_points (type INT),
    liability_usd (type NUMBER(18, 2)),
    expiring_points (type INT),
    average_points_per_member (type NUMBER(12, 2)),
  ),
);

WITH balances AS (
  SELECT *
  FROM __ref("int_loyalty_balances")
),

latest AS (
  SELECT
    b.loyalty_account_id,
    COALESCE(b.tier_code, 'NONE') AS tier_code,
    b.running_points,
    b.running_value_usd,
    b.expiring_points
  FROM balances AS b
  WHERE b.posted_at <= '@@valuation_date'::TIMESTAMP_TZ
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY b.loyalty_account_id
      ORDER BY b.posted_at DESC, b.entry_id DESC
    ) = 1
),

final AS (
  SELECT
    l.tier_code::VARCHAR AS tier_code,
    COUNT(DISTINCT l.loyalty_account_id)::INT AS member_count,
    SUM(l.running_points)::INT AS outstanding_points,
    ROUND(SUM(l.running_value_usd), 2)::NUMBER(18, 2) AS liability_usd,
    SUM(l.expiring_points)::INT AS expiring_points,
    ROUND(AVG(l.running_points), 2)::NUMBER(12, 2) AS average_points_per_member
  FROM latest AS l
  GROUP BY
    l.tier_code
)

SELECT *
FROM final
