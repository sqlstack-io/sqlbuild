MODEL (
  description """
    One row per SKU, warehouse and calendar day with the on-hand quantity at the end of the
    warehouse's local day (ASOF the stock ledger) and the most recent cycle count before it.
  """,
  columns (
    stock_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    calendar_date (type DATE, audits [not_null]),
    month_start (type DATE),
    on_hand_qty (type INT),
    last_counted_qty (type INT),
    last_counted_date (type DATE),
    days_since_count (type INT),
    is_stocked_out (type BOOLEAN),
  ),
);

WITH ledger AS (
  SELECT *
  FROM __ref("int_inventory_ledger")
),

counts AS (
  SELECT *
  FROM __ref("stg_stock_counts")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

@sql.date_spine(@const("history_start_date"), 730)

stock_keys AS (
  SELECT
    l.sku,
    l.warehouse_code,
    MIN(l.occurred_date_local) AS first_date
  FROM ledger AS l
  GROUP BY
    l.sku,
    l.warehouse_code
), key_days AS (
  SELECT
    k.sku,
    k.warehouse_code,
    d.calendar_date,
    CONVERT_TIMEZONE(
      w.iana_timezone,
      'UTC',
      DATEADD('day', 1, d.calendar_date)::TIMESTAMP_NTZ
    )::TIMESTAMP_NTZ AS day_end_utc,
    DATEADD('day', 1, d.calendar_date)::TIMESTAMP_NTZ AS day_end_local
  FROM stock_keys AS k
  INNER JOIN date_spine AS d
    ON d.calendar_date >= k.first_date
  INNER JOIN warehouses AS w
    ON k.warehouse_code = w.warehouse_code
  WHERE d.calendar_date <= '@@valuation_date'::DATE
), with_balance AS (
  SELECT
    kd.sku,
    kd.warehouse_code,
    kd.calendar_date,
    kd.day_end_local,
    l.running_on_hand
  FROM key_days AS kd
  ASOF JOIN ledger AS l MATCH_CONDITION (kd.day_end_utc > l.occurred_at_utc)
    ON kd.sku = l.sku
    AND kd.warehouse_code = l.warehouse_code
), with_count AS (
  SELECT
    b.sku,
    b.warehouse_code,
    b.calendar_date,
    b.running_on_hand,
    c.counted_qty,
    c.counted_at_local
  FROM with_balance AS b
  ASOF JOIN counts AS c MATCH_CONDITION (b.day_end_local > c.counted_at_local)
    ON b.sku = c.sku
    AND b.warehouse_code = c.warehouse_code
), final AS (
  SELECT
    (
      c.sku || '|' || c.warehouse_code || '|' || TO_CHAR(c.calendar_date, 'YYYY-MM-DD')
    )::VARCHAR AS stock_day_key,
    c.sku,
    c.warehouse_code,
    c.calendar_date,
    DATE_FROM_PARTS(YEAR(c.calendar_date), MONTH(c.calendar_date), 1) AS month_start,
    COALESCE(c.running_on_hand, 0)::INT AS on_hand_qty,
    c.counted_qty::INT AS last_counted_qty,
    c.counted_at_local::DATE AS last_counted_date,
    DATEDIFF(
      'day',
      c.counted_at_local::DATE,
      c.calendar_date
    )::INT AS days_since_count,
    (
      COALESCE(c.running_on_hand, 0) <= 0
    ) AS is_stocked_out
  FROM with_count AS c
)
SELECT *
FROM final
