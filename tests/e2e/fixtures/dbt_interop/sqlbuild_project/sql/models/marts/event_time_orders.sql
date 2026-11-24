MODEL (
  description "Test model event_time_orders.",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor ordered_at,
  cursor_type timestamp,
  cursor_grain day,
  tags [event_time],
);

SELECT order_id, ordered_at FROM __dbt_ref("analytics", "fact_orders")
