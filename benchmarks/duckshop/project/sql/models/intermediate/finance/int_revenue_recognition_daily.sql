MODEL (
  description """
    One row per recognition day, region and revenue stream. Product revenue is recognised on
    delivery in the region's reporting time zone, refunds reverse it on the refund date, and
    subscription revenue accrues daily. Includes month-to-date and trailing seven-day totals.
  """,
  columns (
    recognition_key (type VARCHAR, nullable false, audits [not_null, unique]),
    recognition_date (type DATE, audits [not_null]),
    region_code (type VARCHAR),
    revenue_stream (type VARCHAR, audits [
      not_null,
      accepted_values (values ["product", "refund", "subscription"]),
    ]),
    revenue_usd (type NUMBER(18, 2)),
    document_count (type INT),
    month_to_date_usd (type NUMBER(18, 2)),
    trailing_7d_usd (type NUMBER(18, 2)),
    share_of_day (type NUMBER(9, 6)),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

timelines AS (
  SELECT *
  FROM __ref("int_order_status_timeline")
),

refunds AS (
  SELECT *
  FROM __ref("int_refunds_usd")
),

subscription_days AS (
  SELECT *
  FROM __ref("int_subscription_mrr_daily")
),

customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

delivered_orders AS (
  SELECT
    o.order_id,
    o.region_code,
    o.amount_usd - COALESCE(o.tax_amount_usd, 0) AS recognisable_usd,
    t.first_delivered_at
  FROM orders AS o
  INNER JOIN timelines AS t
    ON o.order_id = t.order_id
  WHERE t.first_delivered_at IS NOT NULL
),

order_regions AS (
  SELECT
    o.order_id,
    o.region_code
  FROM orders AS o
),

subscription_regions AS (
  SELECT
    d.subscription_day_key,
    d.calendar_date,
    d.mrr_usd,
    c.region_code
  FROM subscription_days AS d
  LEFT JOIN customers AS c
    ON d.customer_id = c.customer_id
),

recognition_events AS (
  SELECT
    CONVERT_TIMEZONE(
      COALESCE(@const("reporting_timezone_by_region")[d.region_code]::VARCHAR, 'UTC'),
      d.first_delivered_at
    )::DATE AS recognition_date,
    d.region_code::VARCHAR AS region_code,
    'product'::VARCHAR AS revenue_stream,
    d.recognisable_usd::NUMBER(18, 4) AS amount_usd,
    d.order_id::VARCHAR AS document_id
  FROM delivered_orders AS d
  UNION ALL
  SELECT
    r.refunded_date::DATE AS recognition_date,
    o.region_code::VARCHAR AS region_code,
    'refund'::VARCHAR AS revenue_stream,
    (
      -r.amount_usd
    )::NUMBER(18, 4) AS amount_usd,
    r.refund_id::VARCHAR AS document_id
  FROM refunds AS r
  LEFT JOIN order_regions AS o
    ON r.order_id = o.order_id
  UNION ALL
  SELECT
    s.calendar_date::DATE AS recognition_date,
    s.region_code::VARCHAR AS region_code,
    'subscription'::VARCHAR AS revenue_stream,
    (
      s.mrr_usd * 12 / 365
    )::NUMBER(18, 4) AS amount_usd,
    s.subscription_day_key::VARCHAR AS document_id
  FROM subscription_regions AS s
),

daily AS (
  SELECT
    e.recognition_date,
    COALESCE(e.region_code, 'UNKNOWN') AS region_code,
    e.revenue_stream,
    SUM(e.amount_usd) AS revenue_usd,
    COUNT(DISTINCT e.document_id) AS document_count
  FROM recognition_events AS e
  WHERE e.recognition_date <= '@@valuation_date'::DATE
  GROUP BY
    e.recognition_date,
    COALESCE(e.region_code, 'UNKNOWN'),
    e.revenue_stream
),

rolling AS (
  SELECT
    d.recognition_date,
    d.region_code,
    d.revenue_stream,
    d.revenue_usd,
    d.document_count,
    SUM(d.revenue_usd) OVER (
      PARTITION BY d.region_code, d.revenue_stream, DATE_TRUNC('month', d.recognition_date)
      ORDER BY d.recognition_date
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS month_to_date_usd,
    SUM(d.revenue_usd) OVER (
      PARTITION BY d.region_code, d.revenue_stream
      ORDER BY d.recognition_date
      RANGE BETWEEN INTERVAL '6 days' PRECEDING AND CURRENT ROW
    ) AS trailing_7d_usd,
    RATIO_TO_REPORT(d.revenue_usd) OVER (
      PARTITION BY d.recognition_date, d.region_code
    ) AS share_of_day
  FROM daily AS d
),

final AS (
  SELECT
    (
      TO_VARCHAR(r.recognition_date, 'YYYY-MM-DD')
        || ':'
        || r.region_code
        || ':'
        || r.revenue_stream
    )::VARCHAR AS recognition_key,
    r.recognition_date,
    r.region_code::VARCHAR AS region_code,
    r.revenue_stream,
    ROUND(r.revenue_usd, 2)::NUMBER(18, 2) AS revenue_usd,
    r.document_count::INT AS document_count,
    ROUND(r.month_to_date_usd, 2)::NUMBER(18, 2) AS month_to_date_usd,
    ROUND(r.trailing_7d_usd, 2)::NUMBER(18, 2) AS trailing_7d_usd,
    ROUND(r.share_of_day, 6)::NUMBER(9, 6) AS share_of_day
  FROM rolling AS r
)

SELECT *
FROM final
