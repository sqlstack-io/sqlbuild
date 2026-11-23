MODEL (
  materialized incremental,
  incremental_strategy delete_insert,
  cursor order_id,
  cursor_type integer,
  on_schema_change append_new_columns,
  replay_on_change bounded-7d,
  tags [intermediate, acceptance],
  description "Integer-cursor incremental projection over fact orders.",
  columns (
    order_id (audits [not_null, unique]),
  ),
);

SELECT
  order_id,
  customer_id,
  order_status,
  ordered_at,
  line_total_cents
FROM __ref("fact_orders")
