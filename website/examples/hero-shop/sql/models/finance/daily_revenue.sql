MODEL (
  description "Daily revenue from successful payments",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor revenue_date,
  cursor_type timestamp,
  cursor_grain day,
  cursor_inputs (
    payments paid_at,
  ),
);

SELECT
  CAST(paid_at AS DATE) AS revenue_date,
  SUM(amount_cents) AS total_revenue_cents
FROM __seed("payments")
GROUP BY
  CAST(paid_at AS DATE)
