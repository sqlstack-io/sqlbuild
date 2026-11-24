MODEL (
  description """
    One row per warehouse and local work date with picking throughput and delivery slot utilisation
  """,
  columns (
    productivity_key (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR),
    work_date_local (type DATE),
    pickers (type INT),
    units_picked (type INT),
    units_per_picker_hour (type NUMBER(10, 2)),
    top_picker_id (type VARCHAR),
    slot_utilisation (type NUMBER(9, 4)),
    is_holiday (type BOOLEAN),
  ),
);

WITH picks AS (
  SELECT *
  FROM __ref("int_pick_performance")
),

slots AS (
  SELECT *
  FROM __ref("int_delivery_slot_utilisation")
),

daily_picks AS (
  SELECT
    p.warehouse_code,
    p.work_date_local,
    COUNT(DISTINCT p.picker_id) AS pickers,
    SUM(p.units_picked) AS units_picked,
    SUM(p.active_hours) AS active_hours,
    MAX_BY(p.picker_id, p.units_per_hour) AS top_picker_id,
    BOOLOR_AGG(p.is_holiday) AS is_holiday
  FROM picks AS p
  GROUP BY
    p.warehouse_code,
    p.work_date_local
),

final AS (
  SELECT
    (
      d.warehouse_code || ':' || TO_VARCHAR(d.work_date_local, 'YYYY-MM-DD')
    )::VARCHAR AS productivity_key,
    d.warehouse_code,
    d.work_date_local,
    d.pickers::INT AS pickers,
    d.units_picked::INT AS units_picked,
    ROUND(d.units_picked / NULLIF(d.active_hours, 0), 2)::NUMBER(10, 2) AS units_per_picker_hour,
    d.top_picker_id::VARCHAR AS top_picker_id,
    s.utilisation AS slot_utilisation,
    d.is_holiday::BOOLEAN AS is_holiday
  FROM daily_picks AS d
  LEFT JOIN slots AS s
    ON d.warehouse_code = s.warehouse_code
    AND d.work_date_local = s.slot_date_local
)

SELECT *
FROM final
