MODEL (
  description """
    Version history of warehouse operating attributes, recording a new version when hours or
    capabilities change
  """,
  materialized snapshot,
  unique_key [warehouse_code],
  snapshot_strategy check,
  check_columns [opens_local, closes_local, supports_same_day, supports_cold_chain, max_parcel_kg],
  columns (
    warehouse_code (type VARCHAR, nullable false, audits [not_null]),
    iana_timezone (type VARCHAR),
    opens_local (type TIME),
    closes_local (type TIME),
    supports_same_day (type BOOLEAN),
    supports_cold_chain (type BOOLEAN),
    max_parcel_kg (type NUMBER(6, 2)),
  ),
);

WITH warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

final AS (
  SELECT
    w.warehouse_code,
    w.iana_timezone,
    w.opens_local,
    w.closes_local,
    w.supports_same_day,
    w.supports_cold_chain,
    w.max_parcel_kg
  FROM warehouses AS w
)

SELECT *
FROM final
