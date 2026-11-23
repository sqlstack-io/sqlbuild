MODEL (
  materialized incremental,
  incremental_strategy merge,
  unique_key [customer_id],
  cursor last_ordered_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    fact_orders ordered_at,
  ),
  replay_on_change full,
  on_schema_change sync_all_columns,
  tags [intermediate, acceptance],
  description "Merge-based customer snapshot with timestamp cursor and explicit backfill policies.",
  columns (
    customer_id (audits [not_null, unique]),
  ),
);

SELECT
  customer_id,
  MAX(ordered_at) AS last_ordered_at,
  MAX(order_status) AS latest_order_status,
  COUNT(*) AS total_orders,
  SUM(line_total_cents) AS total_revenue_cents
FROM __ref("fact_orders")
GROUP BY customer_id
