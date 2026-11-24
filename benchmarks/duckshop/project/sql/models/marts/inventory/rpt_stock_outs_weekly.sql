MODEL (
  description """
    One row per ISO week and warehouse with stock-out days, SKUs affected and the longest stock-out
    run
  """,
  columns (
    stock_out_key (type VARCHAR, nullable false, audits [not_null, unique]),
    week_start (type DATE),
    warehouse_code (type VARCHAR),
    sku_days (type INT),
    stocked_out_sku_days (type INT),
    stock_out_rate (type NUMBER(9, 4)),
    skus_affected (type INT),
    longest_run_days (type INT),
  ),
);

WITH inventory AS (
  SELECT *
  FROM __ref("fct_inventory_daily")
),

runs AS (
  SELECT
    i.sku,
    i.warehouse_code,
    i.calendar_date,
    i.is_stocked_out,
    DATEADD(
      'day',
      -COUNT_IF(i.is_stocked_out) OVER (
        PARTITION BY i.sku, i.warehouse_code
        ORDER BY i.calendar_date
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
      ),
      i.calendar_date
    ) AS run_anchor
  FROM inventory AS i
),

run_lengths AS (
  SELECT
    r.warehouse_code,
    DATE_TRUNC('week', MIN(r.calendar_date)) AS week_start,
    COUNT(*) AS run_days
  FROM runs AS r
  WHERE r.is_stocked_out
  GROUP BY
    r.sku,
    r.warehouse_code,
    r.run_anchor
),

weekly AS (
  SELECT
    DATE_TRUNC('week', r.calendar_date) AS week_start,
    r.warehouse_code,
    COUNT(*) AS sku_days,
    COUNT_IF(r.is_stocked_out) AS stocked_out_sku_days,
    COUNT(DISTINCT IFF(r.is_stocked_out, r.sku, NULL)) AS skus_affected
  FROM runs AS r
  GROUP BY
    1,
    2
),

longest AS (
  SELECT
    l.week_start,
    l.warehouse_code,
    MAX(l.run_days) AS longest_run_days
  FROM run_lengths AS l
  GROUP BY
    l.week_start,
    l.warehouse_code
),

final AS (
  SELECT
    (
      w.warehouse_code || ':' || TO_VARCHAR(w.week_start, 'YYYY-MM-DD')
    )::VARCHAR AS stock_out_key,
    w.week_start::DATE AS week_start,
    w.warehouse_code,
    w.sku_days::INT AS sku_days,
    w.stocked_out_sku_days::INT AS stocked_out_sku_days,
    ROUND(w.stocked_out_sku_days / NULLIF(w.sku_days, 0), 4)::NUMBER(9, 4) AS stock_out_rate,
    w.skus_affected::INT AS skus_affected,
    COALESCE(l.longest_run_days, 0)::INT AS longest_run_days
  FROM weekly AS w
  LEFT JOIN longest AS l
    ON w.week_start = l.week_start
    AND w.warehouse_code = l.warehouse_code
)

SELECT *
FROM final
