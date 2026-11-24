MODEL (
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_day,
  cursor_type timestamp,
  cursor_grain day,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor_inputs (
    hourly_order_activity (column activity_hour, roles [filter, watermark]),
  ),
  incremental_mode microbatch,
  batch_size 2d,
  replay_on_change bounded-14d,
  tags [marts, acceptance],
  description "Downstream daily microbatch rollup with a wider batch size than its hourly upstream.",
  columns (
    activity_day (audits [not_null (run_scope delta_and_final)]),
  ),
  audits [
    expression_is_true (
      name "daily_orders_placed_is_non_negative",
      expression "orders_placed >= 0",
      run_scope delta_and_final,
    ),
  ],
);

SELECT
  @py.timestamp_trunc('day', 'activity_hour') AS activity_day,
  SUM(orders_placed) AS orders_placed,
  SUM(waffles_ordered) AS waffles_ordered,
  SUM(revenue_cents) AS revenue_cents
FROM __ref("hourly_order_activity")
GROUP BY @py.timestamp_trunc('day', 'activity_hour')
