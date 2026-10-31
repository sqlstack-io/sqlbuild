MODEL (
  description 'Backfill orders over an explicit integer window.',
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  cursor batch_id,
  cursor_type integer,
  cursor_start 0,
  cursor_watermark_mode all,
  cursor_inputs (raw_orders (column batch_id, roles [filter, watermark])),
  batch_size '2',
  batch_concurrency 1,
);
SELECT amount, batch_id FROM __source("raw_orders")
WHERE batch_id >= __cursor_start() AND batch_id < __cursor_end()
