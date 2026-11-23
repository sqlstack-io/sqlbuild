MODEL (
  description "One row per order, unique by order_id and by order_number.",
  columns (
    order_id (type NUMBER, nullable false, audits [unique]),
    order_number (type VARCHAR, nullable false, audits [unique]),
  ),
);

SELECT
  1 AS order_id,
  'SO-1' AS order_number,
  PARSE_JSON('{"customer": "c1"}') AS header
