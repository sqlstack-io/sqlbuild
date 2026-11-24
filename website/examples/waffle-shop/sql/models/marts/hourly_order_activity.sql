MODEL (
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor_inputs (
    fact_orders (column ordered_at, roles [filter, watermark]),
  ),
  incremental_mode microbatch,
  batch_size 1d,
  tags [marts],
  description "Hourly order activity aggregated via microbatch incremental.",
  columns (
    activity_hour (audits [not_null (run_scope delta_and_final)]),
  ),
  audits [
    expression_is_true (
      name "orders_placed_is_non_negative",
      expression "orders_placed >= 0",
      run_scope delta_and_final,
    ),
  ],
);

SELECT
  @py.timestamp_trunc('hour', 'o.ordered_at') AS activity_hour,
  COUNT(*) AS orders_placed,
  SUM(o.quantity) AS waffles_ordered,
  SUM(o.line_total_cents) AS revenue_cents
FROM __ref("fact_orders") o
GROUP BY @py.timestamp_trunc('hour', 'o.ordered_at')
