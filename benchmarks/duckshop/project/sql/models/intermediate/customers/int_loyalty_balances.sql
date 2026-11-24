MODEL (
  description """
    One row per loyalty ledger posting with the account's running points balance, the points that
    expire unused, and the balance valued at the programme rate.
  """,
  constants (
    _point_value_usd constant(type decimal, value "0.0100"),
  ),
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    loyalty_account_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    posted_at (type TIMESTAMP_TZ),
    points_delta (type INT),
    running_points (type INT),
    expiring_points (type INT),
    is_expired (type BOOLEAN),
    running_value_usd (type NUMBER(18, 4)),
    reason_code (type VARCHAR),
    tier_code (type VARCHAR),
  ),
);

WITH ledger AS (
  SELECT *
  FROM __ref("stg_loyalty_ledger")
),

accounts AS (
  SELECT *
  FROM __ref("stg_loyalty_accounts")
),

postings AS (
  SELECT
    l.entry_id,
    l.loyalty_account_id,
    l.posted_at,
    l.points_delta,
    l.reason_code,
    l.expires_on,
    SUM(l.points_delta) OVER (
      PARTITION BY l.loyalty_account_id
      ORDER BY l.posted_at, l.entry_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS running_points
  FROM ledger AS l
),

expiry AS (
  SELECT
    p.entry_id,
    IFF(
      p.points_delta > 0 AND p.expires_on < '@@valuation_date'::DATE,
      p.points_delta,
      0
    ) AS expiring_points
  FROM postings AS p
),

final AS (
  SELECT
    p.entry_id,
    p.loyalty_account_id,
    a.customer_id,
    p.posted_at,
    p.points_delta,
    p.running_points::INT AS running_points,
    e.expiring_points::INT AS expiring_points,
    (
      e.expiring_points > 0
    ) AS is_expired,
    ROUND(p.running_points * @const("_point_value_usd"), 4)::NUMBER(18, 4) AS running_value_usd,
    p.reason_code,
    a.tier_code
  FROM postings AS p
  INNER JOIN expiry AS e
    ON p.entry_id = e.entry_id
  LEFT JOIN accounts AS a
    ON p.loyalty_account_id = a.loyalty_account_id
)

SELECT *
FROM final
