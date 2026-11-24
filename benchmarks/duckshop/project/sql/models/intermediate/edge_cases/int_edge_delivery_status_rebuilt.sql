MODEL (
  description """
    One row per shipment with the state of its latest delivery version over the whole change log,
    deletes kept as flagged rows: the full-history reference the incremental delivery status must
    equal after any sequence of loads
  """,
  tags [edge_case],
  materialized view,
  columns (
    shipment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_ref (type VARCHAR),
    delivery_status (type VARCHAR),
    ordered_on (type DATE),
    delivered_on (type DATE),
    is_deleted (type BOOLEAN),
    latest_seq (type NUMBER(38, 0)),
    source_loaded_at (type TIMESTAMP_NTZ),
  ),
);

WITH updates AS (
  SELECT
    u.delivery_id,
    u.shipment_id,
    u.order_ref,
    u.delivery_status,
    u.ordered_on,
    u.delivered_on,
    u._op,
    u._seq,
    MAX(u._loaded_at) OVER (PARTITION BY u.shipment_id) AS source_loaded_at
  FROM __source("edge__delivery_updates") AS u
),

final AS (
  SELECT
    u.shipment_id,
    u.order_ref,
    u.delivery_status,
    u.ordered_on,
    u.delivered_on,
    (
      u._op = 'D'
    ) AS is_deleted,
    u._seq AS latest_seq,
    u.source_loaded_at
  FROM updates AS u
  QUALIFY
    ROW_NUMBER() OVER (PARTITION BY u.shipment_id ORDER BY u._seq DESC, u.delivery_id DESC) = 1
)

SELECT *
FROM final
