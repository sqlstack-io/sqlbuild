MODEL (
  description 'Backfill orders over an explicit timestamp window.',
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  cursor ordered_at,
  cursor_type timestamp,
  cursor_grain day,
  cursor_start '2026-07-01',
  cursor_watermark_mode all,
  cursor_inputs (raw_orders (column ordered_at, roles [filter, watermark])),
  batch_size 1d,
  batch_concurrency 1,
);
SELECT amount, ordered_at FROM __source("raw_orders")
WHERE ordered_at >= __cursor_start() AND ordered_at < __cursor_end()
