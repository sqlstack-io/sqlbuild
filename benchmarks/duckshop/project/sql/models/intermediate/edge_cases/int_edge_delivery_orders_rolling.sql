MODEL (
  description """
    One row per delivery version ordered since February 2026, built in rolling-window microbatches
    by order date. A first build or full refresh starts at the declared cursor_start, not at the
    lookback, and every build ends with the current day.
  """,
  tags [edge_case],
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy rolling_window,
  cursor ordered_on,
  cursor_type timestamp,
  cursor_grain day,
  cursor_start "2026-02-01",
  batch_size 30d,
  lookback 2d,
  cursor_inputs (
    edge__delivery_updates ordered_on,
  ),
  columns (
    delivery_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR),
    ordered_on (type DATE, nullable false, audits [not_null]),
  ),
);

WITH updates AS (
  SELECT
    u.delivery_id,
    u.shipment_id,
    u.ordered_on
  FROM __source("edge__delivery_updates") AS u
),

final AS (
  SELECT
    u.delivery_id,
    u.shipment_id,
    u.ordered_on
  FROM updates AS u
  WHERE
    u.ordered_on >= __cursor_start()
    AND u.ordered_on < __cursor_end()
)

SELECT *
FROM final
