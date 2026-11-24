MODEL (
  description """
    One row per picker, warehouse and local work date with pick throughput and its rank within the
    warehouse that day
  """,
  columns (
    picker_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    picker_id (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR),
    work_date_local (type DATE),
    task_count (type INT),
    units_picked (type INT),
    active_hours (type NUMBER(10, 2)),
    units_per_hour (type NUMBER(10, 2)),
    p90_task_seconds (type INT),
    rank_in_warehouse (type INT),
    first_task_hour_local (type INT),
    shift_coverage (type NUMBER(9, 4)),
    is_holiday (type BOOLEAN),
  ),
);

WITH pick_tasks AS (
  SELECT *
  FROM __ref("stg_pick_tasks")
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

warehouse_context AS (
  SELECT
    w.warehouse_code,
    w.opening_minutes,
    c.region_code
  FROM warehouses AS w
  LEFT JOIN countries AS c
    ON w.country_code = c.country_code
),

completed AS (
  SELECT
    t.picker_id,
    t.warehouse_code,
    t.started_at_local,
    t.started_at_local::DATE AS work_date_local,
    t.pick_seconds,
    t.units_picked
  FROM pick_tasks AS t
  WHERE
    t.completed_at_local IS NOT NULL
    AND t.pick_seconds > 0
),

daily AS (
  SELECT
    c.picker_id,
    c.warehouse_code,
    c.work_date_local,
    COUNT(*) AS task_count,
    SUM(c.units_picked) AS units_picked,
    SUM(c.pick_seconds) / 3600 AS active_hours,
    PERCENTILE_DISC(0.9) WITHIN GROUP (ORDER BY c.pick_seconds) AS p90_task_seconds,
    MIN(HOUR(c.started_at_local)) AS first_task_hour_local
  FROM completed AS c
  GROUP BY
    c.picker_id,
    c.warehouse_code,
    c.work_date_local
),

with_context AS (
  SELECT
    d.picker_id,
    d.warehouse_code,
    d.work_date_local,
    d.task_count,
    d.units_picked,
    d.active_hours,
    d.p90_task_seconds,
    d.first_task_hour_local,
    60 * d.active_hours / NULLIF(w.opening_minutes, 0) AS shift_coverage,
    (
      h.holiday_date IS NOT NULL
    ) AS is_holiday
  FROM daily AS d
  LEFT JOIN warehouse_context AS w
    ON d.warehouse_code = w.warehouse_code
  LEFT JOIN holidays AS h
    ON w.region_code = h.region_code
    AND d.work_date_local = h.holiday_date
),

final AS (
  SELECT
    (
      d.picker_id || '|' || d.warehouse_code || '|' || TO_CHAR(d.work_date_local, 'YYYY-MM-DD')
    )::VARCHAR AS picker_day_key,
    d.picker_id,
    d.warehouse_code,
    d.work_date_local,
    d.task_count::INT AS task_count,
    d.units_picked::INT AS units_picked,
    ROUND(d.active_hours, 2)::NUMBER(10, 2) AS active_hours,
    ROUND(d.units_picked / NULLIF(d.active_hours, 0), 2)::NUMBER(10, 2) AS units_per_hour,
    d.p90_task_seconds::INT AS p90_task_seconds,
    DENSE_RANK() OVER (
      PARTITION BY d.warehouse_code, d.work_date_local
      ORDER BY d.units_picked / NULLIF(d.active_hours, 0) DESC NULLS LAST
    )::INT AS rank_in_warehouse,
    d.first_task_hour_local::INT AS first_task_hour_local,
    ROUND(d.shift_coverage, 4)::NUMBER(9, 4) AS shift_coverage,
    d.is_holiday::BOOLEAN AS is_holiday
  FROM with_context AS d
)

SELECT *
FROM final
