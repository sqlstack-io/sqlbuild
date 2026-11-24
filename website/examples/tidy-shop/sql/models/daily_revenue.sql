MODEL (
  description "Daily revenue from successful payments",
  materialized table,
);

SELECT CAST('2026-04-01' AS DATE) AS revenue_date, 2850 AS total_revenue_cents
