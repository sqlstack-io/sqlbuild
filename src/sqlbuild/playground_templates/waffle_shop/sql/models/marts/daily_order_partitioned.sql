MODEL (
  materialized partition_tracked,
  tags [marts],
  placeholders (
    partition_start "'2026-04-01'",
    partition_end "'2026-04-05'",
  ),
  config (
    tracking_table partition_state,
    partition_column order_date,
    date_range_start 2026-04-01,
    date_range_end 2026-04-05,
  ),
  description "Partition-tracked daily order summary using custom materialization.",
  columns (
    order_date (audits [not_null]),
  ),
  audits [
    expression_is_true (
      name "waffles_ordered_is_positive",
      expression "waffles_ordered > 0",
    ),
  ],
);

SELECT
  CAST(o.ordered_at AS DATE) AS order_date,
  COUNT(DISTINCT o.order_id) AS order_count,
  SUM(o.quantity) AS waffles_ordered,
  COUNT(DISTINCT o.customer_id) AS unique_customers
FROM __ref("stg_orders") o
WHERE CAST(o.ordered_at AS DATE) >= CAST(@@@partition_start AS DATE)
  AND CAST(o.ordered_at AS DATE) < CAST(@@@partition_end AS DATE)
GROUP BY CAST(o.ordered_at AS DATE)
