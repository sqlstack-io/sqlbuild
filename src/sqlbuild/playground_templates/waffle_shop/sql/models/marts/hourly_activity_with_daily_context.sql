MODEL (
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor_inputs (
    hourly_order_activity (column activity_hour, roles [filter]),
    daily_activity_rollup (column activity_day, roles [filter, watermark]),
  ),
  incremental_mode microbatch,
  batch_size 6h,
  tags [marts, acceptance],
  description "Hourly downstream microbatch model that depends on a coarser-grain daily upstream.",
  columns (
    activity_hour (audits [not_null (run_scope delta_and_final)]),
  ),
  audits [
    expression_is_true (
      name "day_orders_cover_hourly_orders",
      expression "day_orders_placed >= orders_placed",
      run_scope delta_and_final,
    ),
  ],
);

WITH hourly_activity AS (
  SELECT
    activity_hour,
    @py.timestamp_trunc('day', 'activity_hour') AS activity_day,
    orders_placed,
    waffles_ordered,
    revenue_cents
  FROM __ref("hourly_order_activity")
), final AS (
  SELECT
    h.activity_hour,
    h.orders_placed,
    d.orders_placed AS day_orders_placed,
    h.waffles_ordered,
    h.revenue_cents
  FROM hourly_activity AS h
  INNER JOIN __ref("daily_activity_rollup") AS d
    ON h.activity_day = d.activity_day
)
SELECT * FROM final
