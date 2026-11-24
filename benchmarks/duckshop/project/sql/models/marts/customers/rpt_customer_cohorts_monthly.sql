MODEL (
  description """
    One row per first-order cohort month and months since first order, with active customers,
    retention against the cohort size and cumulative revenue per cohort customer.
  """,
  columns (
    cohort_cell_key (type VARCHAR, nullable false, audits [not_null, unique]),
    cohort_month (type DATE, audits [not_null]),
    months_since_first_order (type INT, audits [non_negative]),
    cohort_size (type INT),
    active_customers (type INT),
    retention_rate (type NUMBER(9, 4)),
    revenue_usd (type NUMBER(18, 2)),
    cumulative_revenue_per_customer_usd (type NUMBER(18, 2)),
  ),
);

WITH order_sequence AS (
  SELECT *
  FROM __ref("int_customer_order_sequence")
),

activity AS (
  SELECT
    s.customer_id,
    s.cohort_month,
    DATEDIFF(
      'month',
      s.cohort_month,
      DATE_TRUNC('month', s.placed_date_local)
    ) AS months_since_first_order,
    s.amount_usd
  FROM order_sequence AS s
),

cohort_sizes AS (
  SELECT
    a.cohort_month,
    COUNT(DISTINCT a.customer_id) AS cohort_size
  FROM activity AS a
  GROUP BY
    a.cohort_month
),

cells AS (
  SELECT
    a.cohort_month,
    a.months_since_first_order,
    COUNT(DISTINCT a.customer_id) AS active_customers,
    SUM(a.amount_usd) AS revenue_usd
  FROM activity AS a
  GROUP BY
    a.cohort_month,
    a.months_since_first_order
),

final AS (
  SELECT
    (
      TO_VARCHAR(c.cohort_month, 'YYYY-MM') || '+' || c.months_since_first_order::VARCHAR
    )::VARCHAR AS cohort_cell_key,
    c.cohort_month,
    c.months_since_first_order::INT AS months_since_first_order,
    z.cohort_size::INT AS cohort_size,
    c.active_customers::INT AS active_customers,
    ROUND(c.active_customers / NULLIF(z.cohort_size, 0), 4)::NUMBER(9, 4) AS retention_rate,
    c.revenue_usd::NUMBER(18, 2) AS revenue_usd,
    ROUND(
      SUM(c.revenue_usd) OVER (
        PARTITION BY c.cohort_month
        ORDER BY c.months_since_first_order
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
      )
        / NULLIF(z.cohort_size, 0),
      2
    )::NUMBER(18, 2) AS cumulative_revenue_per_customer_usd
  FROM cells AS c
  INNER JOIN cohort_sizes AS z
    ON c.cohort_month = z.cohort_month
)

SELECT *
FROM final
