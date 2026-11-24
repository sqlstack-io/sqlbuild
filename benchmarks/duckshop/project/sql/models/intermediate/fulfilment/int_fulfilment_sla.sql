MODEL (
  description """
    One row per shipped order with dispatch time measured in the shipping warehouse's business hours
    (opening hours, weekends and regional holidays excluded) against a one-business-day dispatch
    promise, and delivery time from the tracking feed.
  """,
  constants (
    _dispatch_promise_business_minutes 480,
  ),
  columns (
    shipment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    carrier_code (type VARCHAR),
    placed_at_local (type TIMESTAMP_NTZ),
    shipped_at_local (type TIMESTAMP_NTZ),
    calendar_days_to_ship (type INT),
    business_days_to_ship (type INT),
    holidays_in_window (type INT),
    dispatch_business_minutes (type INT),
    is_dispatch_on_time (type BOOLEAN),
    delivered_at_utc (type TIMESTAMP_NTZ),
    hours_to_deliver (type NUMBER(10, 2)),
  ),
);

WITH shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_enriched")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

tracking AS (
  SELECT *
  FROM __ref("int_shipment_tracking_events")
),

countries AS (
  SELECT *
  FROM __seed("countries")
),

holidays AS (
  SELECT *
  FROM __seed("holiday_calendar")
),

@sql.date_spine(@const("history_start_date"), 730)

local_times AS (
  SELECT
    s.shipment_id,
    s.order_id,
    s.warehouse_code,
    s.carrier_code,
    c.region_code,
    w.opening_minutes,
    CONVERT_TIMEZONE(w.iana_timezone, o.placed_at)::TIMESTAMP_NTZ AS placed_at_local,
    CONVERT_TIMEZONE(w.iana_timezone, s.shipped_at)::TIMESTAMP_NTZ AS shipped_at_local,
    CONVERT_TIMEZONE(w.iana_timezone, o.placed_at)::DATE AS placed_date_local,
    CONVERT_TIMEZONE(w.iana_timezone, s.shipped_at)::DATE AS shipped_date_local
  FROM shipments AS s
  INNER JOIN orders AS o
    ON s.order_id = o.order_id
  INNER JOIN warehouses AS w
    ON s.warehouse_code = w.warehouse_code
  LEFT JOIN countries AS c
    ON w.country_code = c.country_code
  WHERE s.shipped_at IS NOT NULL
), window_days AS (
  SELECT
    l.shipment_id,
    DAYOFWEEKISO(d.calendar_date) AS iso_weekday,
    h.holiday_date
  FROM local_times AS l
  INNER JOIN date_spine AS d
    ON d.calendar_date > l.placed_date_local
    AND d.calendar_date <= l.shipped_date_local
  LEFT JOIN holidays AS h
    ON l.region_code = h.region_code
    AND d.calendar_date = h.holiday_date
), business_days AS (
  SELECT
    wd.shipment_id,
    COUNT_IF(wd.iso_weekday <= 5 AND wd.holiday_date IS NULL) AS business_days_to_ship,
    COUNT_IF(wd.holiday_date IS NOT NULL) AS holidays_in_window
  FROM window_days AS wd
  GROUP BY
    wd.shipment_id
), delivery AS (
  SELECT
    t.shipment_id,
    MIN(t.event_at_utc) AS delivered_at_utc
  FROM tracking AS t
  WHERE t.is_delivery_event
  GROUP BY
    t.shipment_id
), final AS (
  SELECT
    l.shipment_id,
    l.order_id,
    l.warehouse_code,
    l.carrier_code,
    l.placed_at_local,
    l.shipped_at_local,
    DATEDIFF('day', l.placed_at_local, l.shipped_at_local)::INT AS calendar_days_to_ship,
    COALESCE(b.business_days_to_ship, 0)::INT AS business_days_to_ship,
    COALESCE(b.holidays_in_window, 0)::INT AS holidays_in_window,
    IFF(
      COALESCE(b.business_days_to_ship, 0) = 0,
      DATEDIFF('minute', l.placed_at_local, l.shipped_at_local),
      COALESCE(b.business_days_to_ship, 0) * l.opening_minutes
    )::INT AS dispatch_business_minutes,
    (
      IFF(
        COALESCE(b.business_days_to_ship, 0) = 0,
        DATEDIFF('minute', l.placed_at_local, l.shipped_at_local),
        COALESCE(b.business_days_to_ship, 0) * l.opening_minutes
      ) <= @const("_dispatch_promise_business_minutes")
    ) AS is_dispatch_on_time,
    dl.delivered_at_utc,
    ROUND(
      DATEDIFF(
        'minute',
        CONVERT_TIMEZONE(w.iana_timezone, 'UTC', l.shipped_at_local),
        dl.delivered_at_utc
      )
      / 60,
      2
    )::NUMBER(10, 2) AS hours_to_deliver
  FROM local_times AS l
  INNER JOIN warehouses AS w
    ON l.warehouse_code = w.warehouse_code
  LEFT JOIN business_days AS b
    ON l.shipment_id = b.shipment_id
  LEFT JOIN delivery AS dl
    ON l.shipment_id = dl.shipment_id
)
SELECT *
FROM final
