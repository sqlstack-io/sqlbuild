MODEL (
  description """
    One row per delivered version of a shipment's delivery change log, every load included, for
    models that recompute a shipment from its whole history
  """,
  tags [edge_case],
  materialized view,
  columns (
    delivery_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, nullable false, audits [not_null]),
    order_ref (type VARCHAR),
    delivery_status (type VARCHAR),
    ordered_on (type DATE),
    delivered_on (type DATE),
    is_delete (type BOOLEAN),
    version_seq (type NUMBER(38, 0)),
    loaded_at (type TIMESTAMP_NTZ),
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
    u._loaded_at
  FROM __source("edge__delivery_updates") AS u
),

final AS (
  SELECT
    u.delivery_id,
    u.shipment_id,
    u.order_ref,
    u.delivery_status,
    u.ordered_on,
    u.delivered_on,
    (
      u._op = 'D'
    ) AS is_delete,
    u._seq AS version_seq,
    u._loaded_at AS loaded_at
  FROM updates AS u
)

SELECT *
FROM final
