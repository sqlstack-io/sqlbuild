MODEL (
  description 'Orders that check returns before they build.',
  materialized table,
  tags [marts],
  pre_hooks [inline_sql('SELECT count(*) FROM __source("raw_returns")')],
);

SELECT CAST(order_id AS BIGINT) AS amount FROM __source("raw_orders")
