MODEL (
  description "One row per order with its line SKUs aggregated.",
  columns (
    order_id (type NUMBER, nullable false, audits [unique]),
  ),
);

SELECT
  l.order_id,
  ARRAY_AGG(l.sku) AS skus
FROM __ref("order_lines") l
GROUP BY l.order_id
