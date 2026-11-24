MODEL (
  description "One row per live delivery slot with remaining capacity",
  columns (
    slot_id (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR, audits [not_null]),
    slot_start_local (type TIMESTAMP_NTZ),
    slot_end_local (type TIMESTAMP_NTZ),
    slot_minutes (type INT),
    capacity (type INT),
    booked (type INT),
    remaining_capacity (type INT),
    excluded_postcode_prefixes (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__delivery_slots")
),

@sql.cdc_latest(src, src.key := (slot_id))

final AS (
  SELECT
    s.slot_id,
    UPPER(s.warehouse_code)::VARCHAR AS warehouse_code,
    s.slot_start_local,
    s.slot_end_local,
    TIMESTAMPDIFF('minute', s.slot_start_local, s.slot_end_local)::INT AS slot_minutes,
    s.capacity::INT AS capacity,
    COALESCE(s.booked, 0)::INT AS booked,
    GREATEST(s.capacity - COALESCE(s.booked, 0), 0)::INT AS remaining_capacity,
    COALESCE(
      s.restrictions:excluded_postcode_prefixes,
      ARRAY_CONSTRUCT()
    )::ARRAY AS excluded_postcode_prefixes,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
)
SELECT *
FROM final
