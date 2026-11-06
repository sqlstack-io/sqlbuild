MODEL (
  description """
    One row per order with the count of its paid quantities, read from text as FLOAT and listed
    in change order
  """,
  tags [edge_case],
  columns (
    order_ref (type VARCHAR, nullable false, audits [not_null, unique]),
    paid_quantity_count (type NUMBER(38, 0)),
  ),
);

WITH quantities AS (
  SELECT
    l.order_ref,
    l.status,
    TRY_TO_DOUBLE(l.quantity_text) AS quantity,
    l.changed_at
  FROM __ref("stg_edge_ledger_entries") AS l
),

quantity_lists AS (
  SELECT
    q.order_ref,
    ARRAY_COMPACT(
      ARRAY_AGG(IFF(q.status = 'paid', q.quantity, NULL)) WITHIN GROUP (ORDER BY q.changed_at)
    ) AS paid_quantities
  FROM quantities AS q
  GROUP BY
    q.order_ref
),

final AS (
  SELECT
    q.order_ref,
    ARRAY_SIZE(q.paid_quantities)::NUMBER(38, 0) AS paid_quantity_count
  FROM quantity_lists AS q
)

SELECT *
FROM final
