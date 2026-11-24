AUDIT (
  name "edge_delivery_status_matches_rebuild",
  severity error
);

-- The incremental delivery status must equal a rebuild from the whole change log after every
-- build, whatever order the versions arrived in. Rows are reported from both sides.
WITH incremental AS (
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
),

rebuilt AS (
  SELECT
    r.shipment_id,
    r.order_ref,
    r.delivery_status,
    r.ordered_on,
    r.delivered_on,
    r.is_deleted,
    r.latest_seq,
    r.source_loaded_at
  FROM __ref("int_edge_delivery_status_rebuilt") AS r
),

only_incremental AS (
  SELECT
    i.shipment_id,
    i.order_ref,
    i.delivery_status,
    i.ordered_on,
    i.delivered_on,
    i.is_deleted,
    i.latest_seq,
    i.source_loaded_at
  FROM incremental AS i
  EXCEPT
  SELECT
    r.shipment_id,
    r.order_ref,
    r.delivery_status,
    r.ordered_on,
    r.delivered_on,
    r.is_deleted,
    r.latest_seq,
    r.source_loaded_at
  FROM rebuilt AS r
),

only_rebuilt AS (
  SELECT
    r.shipment_id,
    r.order_ref,
    r.delivery_status,
    r.ordered_on,
    r.delivered_on,
    r.is_deleted,
    r.latest_seq,
    r.source_loaded_at
  FROM rebuilt AS r
  EXCEPT
  SELECT
    i.shipment_id,
    i.order_ref,
    i.delivery_status,
    i.ordered_on,
    i.delivered_on,
    i.is_deleted,
    i.latest_seq,
    i.source_loaded_at
  FROM incremental AS i
),

final AS (
  SELECT
    'incremental' AS side,
    o.shipment_id
  FROM only_incremental AS o
  UNION ALL
  SELECT
    'rebuilt' AS side,
    b.shipment_id
  FROM only_rebuilt AS b
)

SELECT *
FROM final
