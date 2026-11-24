MODEL (
  description """
    One row per order with its largest quantity text, read as a number, less one
  """,
  tags [edge_case],
  columns (
    order_ref (type VARCHAR, nullable false, audits [not_null, unique]),
    quantity_less_one (type NUMBER(38, 6)),
  ),
);

WITH final AS (
  SELECT
    l.order_ref,
    (MAX(l.quantity_text) - 1)::NUMBER(38, 6) AS quantity_less_one
  FROM __ref("stg_edge_ledger_entries") AS l
  GROUP BY
    l.order_ref
)

SELECT *
FROM final
