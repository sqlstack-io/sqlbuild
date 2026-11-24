MODEL (
  description """
    One row per shipment, copied from the merged delivery status by delete and insert. A shipment
    with a later version moves its source_loaded_at out of the window an earlier build wrote, so
    only its unique_key stops the later build from adding a second row for it.
  """,
  tags [edge_case],
  materialized incremental,
  incremental_strategy delete_insert,
  unique_key [shipment_id],
  cursor source_loaded_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    int_edge_delivery_status source_loaded_at,
  ),
  columns (
    shipment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_ref (type VARCHAR),
    delivery_status (type VARCHAR),
    ordered_on (type DATE),
    delivered_on (type DATE),
    is_deleted (type BOOLEAN),
    latest_seq (type NUMBER(38, 0)),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false, audits [not_null]),
  ),
);

WITH final AS (
  SELECT
    s.shipment_id,
    s.order_ref,
    s.delivery_status,
    s.ordered_on,
    s.delivered_on,
    s.is_deleted,
    s.latest_seq,
    s.source_loaded_at
  FROM __ref("int_edge_delivery_status") AS s
  WHERE
    s.source_loaded_at >= __cursor_start()
    AND s.source_loaded_at < __cursor_end()
)

SELECT *
FROM final
