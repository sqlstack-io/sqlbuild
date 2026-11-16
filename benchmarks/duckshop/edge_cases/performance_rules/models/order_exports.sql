MODEL (
  description "Orders read from an export table SQLBuild cannot see.",
  columns (
    order_id (type NUMBER, nullable false, audits [unique]),
  ),
);

SELECT *
FROM RAW.SHOP.ORDER_EXPORTS
