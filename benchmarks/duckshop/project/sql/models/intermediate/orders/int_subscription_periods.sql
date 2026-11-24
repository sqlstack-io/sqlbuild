MODEL (
  description """
    One row per active, paused or cancelled period of a subscription, built as gaps-and-islands over
    its lifecycle events: the subscription starts active, and every change of state starts a new
    period.
  """,
  columns (
    subscription_period_key (type VARCHAR, nullable false, audits [not_null, unique]),
    subscription_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    period_number (type INT),
    period_state (type VARCHAR, audits [accepted_values (values ["active", "paused", "cancelled"])]),
    period_start (type TIMESTAMP_TZ),
    period_end (type TIMESTAMP_TZ),
    period_days (type NUMBER(10, 2)),
    billing_amount_minor (type INT),
    billing_currency_code (type VARCHAR),
    interval_days (type INT),
  ),
);

WITH subscriptions AS (
  SELECT *
  FROM __ref("stg_subscriptions")
),

events AS (
  SELECT *
  FROM __ref("stg_subscription_events")
),

state_events AS (
  SELECT
    e.subscription_id,
    e.event_id,
    e.occurred_at,
    CASE e.event_type
      WHEN 'PAUSE'
      THEN 'paused'
      WHEN 'CANCEL'
      THEN 'cancelled'
      ELSE 'active'
    END AS state
  FROM events AS e
  WHERE e.event_type IN ('PAUSE', 'RESUME', 'CANCEL')
),

changes AS (
  SELECT
    s.subscription_id,
    s.event_id,
    s.occurred_at,
    s.state,
    LAG(s.state, 1, 'active') OVER (
      PARTITION BY s.subscription_id
      ORDER BY s.occurred_at, s.event_id
    ) AS previous_state
  FROM state_events AS s
),

islands AS (
  SELECT
    c.subscription_id,
    c.occurred_at,
    c.state,
    SUM(IFF(c.previous_state <> c.state, 1, 0)) OVER (
      PARTITION BY c.subscription_id
      ORDER BY c.occurred_at, c.event_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS island_number
  FROM changes AS c
),

event_periods AS (
  SELECT
    i.subscription_id,
    i.island_number,
    ANY_VALUE(i.state) AS period_state,
    MIN(i.occurred_at) AS period_start
  FROM islands AS i
  WHERE i.island_number >= 1
  GROUP BY
    i.subscription_id,
    i.island_number
),

bounded AS (
  SELECT
    p.subscription_id,
    p.island_number + 1 AS period_number,
    p.period_state,
    p.period_start,
    LEAD(p.period_start) OVER (
      PARTITION BY p.subscription_id
      ORDER BY p.island_number
    ) AS period_end
  FROM event_periods AS p
),

first_change AS (
  SELECT
    p.subscription_id,
    MIN(p.period_start) AS first_change_at
  FROM event_periods AS p
  GROUP BY
    p.subscription_id
),

final AS (
  SELECT
    (
      s.subscription_id || '#1'
    )::VARCHAR AS subscription_period_key,
    s.subscription_id::VARCHAR AS subscription_id,
    s.customer_id::VARCHAR AS customer_id,
    1::INT AS period_number,
    'active'::VARCHAR AS period_state,
    s.started_at::TIMESTAMP_TZ AS period_start,
    COALESCE(f.first_change_at, s.cancelled_at)::TIMESTAMP_TZ AS period_end,
    ROUND(
      DATEDIFF('hour', s.started_at, COALESCE(f.first_change_at, s.cancelled_at)) / 24,
      2
    )::NUMBER(10, 2) AS period_days,
    s.billing_amount_minor::INT AS billing_amount_minor,
    s.billing_currency_code::VARCHAR AS billing_currency_code,
    s.interval_days::INT AS interval_days
  FROM subscriptions AS s
  LEFT JOIN first_change AS f
    ON s.subscription_id = f.subscription_id
  UNION ALL
  SELECT
    (
      b.subscription_id || '#' || b.period_number::VARCHAR
    )::VARCHAR AS subscription_period_key,
    b.subscription_id::VARCHAR AS subscription_id,
    s.customer_id::VARCHAR AS customer_id,
    b.period_number::INT AS period_number,
    b.period_state::VARCHAR AS period_state,
    b.period_start::TIMESTAMP_TZ AS period_start,
    COALESCE(
      b.period_end,
      IFF(b.period_state = 'cancelled', b.period_start, s.cancelled_at)
    )::TIMESTAMP_TZ AS period_end,
    ROUND(
      DATEDIFF(
        'hour',
        b.period_start,
        COALESCE(b.period_end, IFF(b.period_state = 'cancelled', b.period_start, s.cancelled_at))
      )
      / 24,
      2
    )::NUMBER(10, 2) AS period_days,
    s.billing_amount_minor::INT AS billing_amount_minor,
    s.billing_currency_code::VARCHAR AS billing_currency_code,
    s.interval_days::INT AS interval_days
  FROM bounded AS b
  INNER JOIN subscriptions AS s
    ON b.subscription_id = s.subscription_id
)

SELECT *
FROM final
