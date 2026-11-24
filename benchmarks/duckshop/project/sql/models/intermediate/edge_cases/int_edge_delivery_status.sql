MODEL (
  description """
    One row per shipment with the state of its latest delivery version, deletes kept as flagged
    rows. Merged incrementally: every shipment with a version loaded in the window is recomputed
    from its whole history, read from a model that is not a cursor input and so is not cut to the
    window, which gives late older versions, redeliveries and deletes the same rows as a rebuild.
  """,
  tags [edge_case],
  materialized incremental,
  incremental_strategy merge,
  unique_key [shipment_id],
  cursor source_loaded_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    edge__delivery_updates _loaded_at,
  ),
  replay_on_change full,
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

WITH window_updates AS (
  SELECT u.shipment_id
  FROM __source("edge__delivery_updates") AS u
  WHERE
    u._loaded_at >= __cursor_start()
    AND u._loaded_at < __cursor_end()
),

touched AS (
  SELECT DISTINCT w.shipment_id
  FROM window_updates AS w
),

history AS (
  SELECT
    v.delivery_id,
    v.shipment_id,
    v.order_ref,
    v.delivery_status,
    v.ordered_on,
    v.delivered_on,
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
),

final AS (
  SELECT
    h.shipment_id,
    h.order_ref,
    h.delivery_status,
    h.ordered_on,
    h.delivered_on,
    h.is_delete AS is_deleted,
    h.version_seq AS latest_seq,
    h.source_loaded_at
  FROM history AS h
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY h.shipment_id
      ORDER BY h.version_seq DESC, h.delivery_id DESC
    ) = 1
)

SELECT *
FROM final
