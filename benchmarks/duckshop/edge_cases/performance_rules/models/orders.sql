MODEL (
  description "One row per order with its raw payload.",
  columns (
    order_id (type NUMBER, nullable false, audits [unique]),
  ),
);

SELECT
  1 AS order_id,
  PARSE_JSON('{"customer": "c1", "lines": [{"sku": "a", "options": [{"code": "gift"}]}]}') AS payload
