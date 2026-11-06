MODEL (
  description """
    One row per warehouse and local day with delivery slot capacity, bookings, utilisation and the
    busiest slot
  """,
  columns (
    slot_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR, audits [not_null]),
    slot_date_local (type DATE),
    slot_count (type INT),
    total_capacity (type INT),
    total_booked (type INT),
    utilisation (type NUMBER(9, 4)),
    full_slot_count (type INT),
    busiest_slot_start_local (type TIMESTAMP_NTZ),
    is_weekend (type BOOLEAN),
    region_code (type VARCHAR),
    is_holiday (type BOOLEAN),
    utilisation_vs_weekday_avg (type NUMBER(9, 4)),
  ),
);

WITH slots AS (
  SELECT *
  FROM __ref("stg_delivery_slots")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

countries AS (
  SELECT *
  FROM __seed("countries")
),

holidays AS (
  SELECT *
  FROM __seed("holiday_calendar")
),

warehouse_regions AS (
  SELECT
    w.warehouse_code,
    c.region_code
  FROM warehouses AS w
  INNER JOIN countries AS c
    ON w.country_code = c.country_code
),

busiest_slots AS (
  SELECT
    s.warehouse_code,
    s.slot_start_local::DATE AS slot_date_local,
    s.slot_start_local AS busiest_slot_start_local
  FROM slots AS s
  WHERE s.capacity <> 0
  -- The earliest of equally busy slots, so ties give one answer.
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY s.warehouse_code, s.slot_start_local::DATE
      ORDER BY s.booked / s.capacity DESC, s.slot_start_local, s.slot_id
    ) = 1
),

by_day AS (
  SELECT
    s.warehouse_code,
    s.slot_start_local::DATE AS slot_date_local,
    COUNT(*) AS slot_count,
    SUM(s.capacity) AS total_capacity,
    SUM(s.booked) AS total_booked,
    COUNT_IF(s.remaining_capacity = 0) AS full_slot_count
  FROM slots AS s
  GROUP BY
    s.warehouse_code,
    s.slot_start_local::DATE
),

with_calendar AS (
  SELECT
    d.warehouse_code,
    d.slot_date_local,
    d.slot_count,
    d.total_capacity,
    d.total_booked,
    d.full_slot_count,
    b.busiest_slot_start_local,
    r.region_code,
    (
      h.holiday_date IS NOT NULL
    ) AS is_holiday,
    d.total_booked / NULLIF(d.total_capacity, 0) AS utilisation_raw
  FROM by_day AS d
  LEFT JOIN busiest_slots AS b
    ON d.warehouse_code = b.warehouse_code
    AND d.slot_date_local = b.slot_date_local
  LEFT JOIN warehouse_regions AS r
    ON d.warehouse_code = r.warehouse_code
  LEFT JOIN holidays AS h
    ON r.region_code = h.region_code
    AND d.slot_date_local = h.holiday_date
),

final AS (
  SELECT
    (
      d.warehouse_code || '|' || TO_CHAR(d.slot_date_local, 'YYYY-MM-DD')
    )::VARCHAR AS slot_day_key,
    d.warehouse_code,
    d.slot_date_local,
    d.slot_count::INT AS slot_count,
    d.total_capacity::INT AS total_capacity,
    d.total_booked::INT AS total_booked,
    ROUND(d.utilisation_raw, 4)::NUMBER(9, 4) AS utilisation,
    d.full_slot_count::INT AS full_slot_count,
    d.busiest_slot_start_local::TIMESTAMP_NTZ AS busiest_slot_start_local,
    (
      DAYOFWEEKISO(d.slot_date_local) >= 6
    ) AS is_weekend,
    IFF(
      @sql.in_regions(d.region_code, @const("supported_regions")),
      d.region_code,
      @const("unknown_region_code")
    )::VARCHAR AS region_code,
    d.is_holiday::BOOLEAN AS is_holiday,
    -- The average as an exact decimal sum over a count, so rounding gives one answer.
    ROUND(
      d.utilisation_raw
        - SUM(d.utilisation_raw) OVER (
          PARTITION BY d.warehouse_code, DAYOFWEEKISO(d.slot_date_local)
        )
        / COUNT(d.utilisation_raw) OVER (
          PARTITION BY d.warehouse_code, DAYOFWEEKISO(d.slot_date_local)
        ),
      4
    )::NUMBER(9, 4) AS utilisation_vs_weekday_avg
  FROM with_calendar AS d
)

SELECT *
FROM final
