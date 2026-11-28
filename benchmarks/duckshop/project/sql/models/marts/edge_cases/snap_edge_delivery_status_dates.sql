MODEL (
  description """
    Version history of each shipment's delivery status, versioned by the day its latest version
    was loaded. The snapshot's timestamp is a DATE, so its validity columns are DATE too; across
    the two delivery loads a shipment's earlier version closes on the later load's day.
  """,
  tags [edge_case],
  materialized snapshot,
  unique_key [shipment_id],
  snapshot_strategy timestamp,
  updated_at loaded_on,
  columns (
    shipment_id (type VARCHAR, nullable false, audits [not_null]),
    delivery_status (type VARCHAR),
    is_deleted (type BOOLEAN),
    loaded_on (type DATE, nullable false, audits [not_null]),
  ),
);

WITH status AS (
  SELECT *
  FROM __ref("int_edge_delivery_status")
),

final AS (
  SELECT
    s.shipment_id,
    s.delivery_status,
    s.is_deleted,
    s.source_loaded_at::DATE AS loaded_on
  FROM status AS s
)

SELECT *
FROM final
