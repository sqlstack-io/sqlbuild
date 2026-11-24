MODEL (
  description """
    Every delivered version of each shipment with a version loaded in the window, replaced as a
    group by delete and insert. A shipment has several rows, so replace_key names the group key:
    unique_key would promise one row per shipment.
  """,
  tags [edge_case],
  materialized incremental,
  incremental_strategy delete_insert,
  replace_key [shipment_id],
  cursor source_loaded_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    edge__delivery_updates _loaded_at,
  ),
  columns (
    delivery_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, nullable false, audits [not_null]),
    delivery_status (type VARCHAR),
    is_delete (type BOOLEAN),
    version_seq (type NUMBER(38, 0)),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false, audits [not_null]),
  ),
);

WITH touched AS (
  SELECT DISTINCT u.shipment_id
  FROM __source("edge__delivery_updates") AS u
  WHERE
    u._loaded_at >= __cursor_start()
    AND u._loaded_at < __cursor_end()
),

final AS (
  SELECT
    v.delivery_id,
    v.shipment_id,
    v.delivery_status,
    v.is_delete,
    v.version_seq,
    MAX(v.loaded_at) OVER (PARTITION BY v.shipment_id) AS source_loaded_at
  FROM __ref("int_edge_delivery_update_versions") AS v
  WHERE
    v.loaded_at < __cursor_end()
    AND v.shipment_id IN (
      SELECT t.shipment_id
      FROM touched AS t
    )
)

SELECT *
FROM final
