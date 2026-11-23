MODEL (
  description """
    One row per warehouse with its location, reporting region, opening hours in local time and the
    UTC offset on the valuation date, and current stock and slot capacity.
  """,
  contract enforced,
  model_schema audit_ts,
  columns (
    warehouse_code (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_name (type VARCHAR),
    country_code (type VARCHAR, audits [not_null]),
    region_code (type VARCHAR),
    iana_timezone (type VARCHAR),
    utc_offset_hours (type NUMBER(4, 2)),
    opens_local (type TIME),
    closes_local (type TIME),
    opening_minutes (type INT),
    supports_same_day (type BOOLEAN),
    supports_cold_chain (type BOOLEAN),
    bin_count (type INT),
    units_on_hand (type INT),
    average_slot_utilisation (type NUMBER(9, 4)),
  ),
);

WITH warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

bins AS (
  SELECT *
  FROM __ref("stg_bin_locations")
),

valuation AS (
  SELECT *
  FROM __ref("int_inventory_valuation_daily")
),

slots AS (
  SELECT *
  FROM __ref("int_delivery_slot_utilisation")
),

countries AS (
  SELECT *
  FROM __seed("countries")
),

bin_counts AS (
  SELECT
    b.warehouse_code,
    COUNT(*) AS bin_count
  FROM bins AS b
  GROUP BY
    b.warehouse_code
),

latest_stock AS (
  SELECT
    v.warehouse_code,
    v.units_on_hand
  FROM valuation AS v
  WHERE v.calendar_date <= '@@valuation_date'::DATE
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY v.warehouse_code
      ORDER BY v.calendar_date DESC, v.valuation_key
    ) = 1
),

slot_averages AS (
  SELECT
    s.warehouse_code,
    AVG(s.utilisation) AS average_slot_utilisation
  FROM slots AS s
  GROUP BY
    s.warehouse_code
),

final AS (
  SELECT
    w.warehouse_code,
    w.warehouse_name,
    w.country_code,
    c.region_code::VARCHAR AS region_code,
    w.iana_timezone,
    ROUND(
      DATEDIFF(
        'minute',
        '@@valuation_date'::TIMESTAMP_NTZ,
        CONVERT_TIMEZONE(
          'UTC',
          w.iana_timezone,
          '@@valuation_date'::TIMESTAMP_NTZ
        )
      )
      / 60,
      2
    )::NUMBER(4, 2) AS utc_offset_hours,
    w.opens_local,
    w.closes_local,
    w.opening_minutes,
    w.supports_same_day,
    w.supports_cold_chain,
    COALESCE(b.bin_count, 0)::INT AS bin_count,
    COALESCE(s.units_on_hand, 0)::INT AS units_on_hand,
    ROUND(u.average_slot_utilisation, 4)::NUMBER(9, 4) AS average_slot_utilisation,
    w.source_loaded_at
  FROM warehouses AS w
  LEFT JOIN countries AS c
    ON w.country_code = c.country_code
  LEFT JOIN bin_counts AS b
    ON w.warehouse_code = b.warehouse_code
  LEFT JOIN latest_stock AS s
    ON w.warehouse_code = s.warehouse_code
  LEFT JOIN slot_averages AS u
    ON w.warehouse_code = u.warehouse_code
)

SELECT *
FROM final
