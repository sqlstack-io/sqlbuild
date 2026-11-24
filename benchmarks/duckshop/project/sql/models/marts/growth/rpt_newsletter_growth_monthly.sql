MODEL (
  description """
    One row per month with new subscribers, unsubscribes, prospects and the running subscribed
    audience
  """,
  columns (
    activity_month (type DATE, nullable false, audits [not_null, unique]),
    new_subscribers (type INT),
    new_prospects (type INT),
    unsubscribes (type INT),
    net_growth (type INT),
    running_audience (type INT),
  ),
);

WITH subscribers AS (
  SELECT *
  FROM __ref("int_newsletter_status")
),

events AS (
  SELECT
    DATE_TRUNC(
      'month',
      CONVERT_TIMEZONE('UTC', s.first_subscribed_at)
    )::DATE AS activity_month,
    1 AS subscribed,
    IFF(s.is_prospect, 1, 0) AS prospect,
    0 AS unsubscribed
  FROM subscribers AS s
  WHERE s.first_subscribed_at IS NOT NULL
  UNION ALL
  SELECT
    DATE_TRUNC(
      'month',
      CONVERT_TIMEZONE('UTC', s.last_unsubscribed_at)
    )::DATE AS activity_month,
    0 AS subscribed,
    0 AS prospect,
    1 AS unsubscribed
  FROM subscribers AS s
  WHERE
    s.is_globally_unsubscribed
    AND s.last_unsubscribed_at IS NOT NULL
),

monthly AS (
  SELECT
    e.activity_month,
    SUM(e.subscribed) AS new_subscribers,
    SUM(e.prospect) AS new_prospects,
    SUM(e.unsubscribed) AS unsubscribes
  FROM events AS e
  GROUP BY
    e.activity_month
),

final AS (
  SELECT
    m.activity_month,
    m.new_subscribers::INT AS new_subscribers,
    m.new_prospects::INT AS new_prospects,
    m.unsubscribes::INT AS unsubscribes,
    (
      m.new_subscribers - m.unsubscribes
    )::INT AS net_growth,
    SUM(m.new_subscribers - m.unsubscribes) OVER (
      ORDER BY m.activity_month
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::INT AS running_audience
  FROM monthly AS m
)

SELECT *
FROM final
