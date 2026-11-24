MODEL (
  description """
    One row per month with opening, new, churned and closing monthly recurring revenue in USD and
    the active subscriber count on the last day of the month.
  """,
  columns (
    month_start (type DATE, nullable false, audits [not_null, unique]),
    closing_mrr_usd (type NUMBER(18, 2)),
    opening_mrr_usd (type NUMBER(18, 2)),
    new_mrr_usd (type NUMBER(18, 2)),
    churned_mrr_usd (type NUMBER(18, 2)),
    net_change_usd (type NUMBER(18, 2)),
    active_subscribers (type INT),
    new_subscribers (type INT),
  ),
);

WITH mrr_days AS (
  SELECT *
  FROM __ref("int_subscription_mrr_daily")
),

month_ends AS (
  SELECT
    DATE_TRUNC('month', d.calendar_date) AS month_start,
    d.subscription_id,
    d.mrr_usd
  FROM mrr_days AS d
  WHERE
    d.calendar_date = LAST_DAY(d.calendar_date)
    OR d.calendar_date = '@@valuation_date'::DATE
),

firsts AS (
  SELECT
    DATE_TRUNC('month', d.calendar_date) AS month_start,
    SUM(d.mrr_usd) AS new_mrr_usd,
    COUNT(*) AS new_subscribers
  FROM mrr_days AS d
  WHERE d.is_first_day
  GROUP BY
    DATE_TRUNC('month', d.calendar_date)
),

closing AS (
  SELECT
    m.month_start,
    SUM(m.mrr_usd) AS closing_mrr_usd,
    COUNT(DISTINCT m.subscription_id) AS active_subscribers
  FROM month_ends AS m
  GROUP BY
    m.month_start
),

final AS (
  SELECT
    c.month_start::DATE AS month_start,
    c.closing_mrr_usd::NUMBER(18, 2) AS closing_mrr_usd,
    LAG(c.closing_mrr_usd, 1, 0) OVER (ORDER BY c.month_start)::NUMBER(18, 2) AS opening_mrr_usd,
    COALESCE(f.new_mrr_usd, 0)::NUMBER(18, 2) AS new_mrr_usd,
    GREATEST(
      LAG(c.closing_mrr_usd, 1, 0) OVER (ORDER BY c.month_start)
        + COALESCE(f.new_mrr_usd, 0)
        - c.closing_mrr_usd,
      0
    )::NUMBER(18, 2) AS churned_mrr_usd,
    (
      c.closing_mrr_usd - LAG(c.closing_mrr_usd, 1, 0) OVER (ORDER BY c.month_start)
    )::NUMBER(18, 2) AS net_change_usd,
    c.active_subscribers::INT AS active_subscribers,
    COALESCE(f.new_subscribers, 0)::INT AS new_subscribers
  FROM closing AS c
  LEFT JOIN firsts AS f
    ON c.month_start = f.month_start
)

SELECT *
FROM final
