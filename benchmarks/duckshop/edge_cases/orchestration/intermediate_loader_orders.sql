MODEL (
  description 'Orders read from shipments whose loader needs an intermediate loader.',
  materialized table,
  tags [marts],
);

SELECT CAST(order_id AS BIGINT) AS amount FROM __source("raw_shipments")
