MODEL (
  description """
    One row per supplier with lead-time percentiles, on-time rate and fill rate over its received
    purchase orders
  """,
  columns (
    supplier_code (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_name (type VARCHAR),
    received_po_count (type INT),
    lead_time_p50_days (type NUMBER(10, 2)),
    lead_time_p90_days (type NUMBER(10, 2)),
    lead_time_stddev_days (type NUMBER(10, 2)),
    on_time_rate (type NUMBER(9, 4)),
    avg_fill_rate (type NUMBER(9, 4)),
    latest_po_at (type TIMESTAMP_TZ),
  ),
);

WITH purchase_orders AS (
  SELECT *
  FROM __ref("stg_purchase_orders")
),

po_lines AS (
  SELECT *
  FROM __ref("int_purchase_order_lines_received")
),

suppliers AS (
  SELECT *
  FROM __ref("stg_suppliers")
),

po_fill AS (
  SELECT
    l.po_id,
    SUM(l.received_qty) / NULLIF(SUM(l.ordered_qty), 0) AS po_fill_rate,
    MAX(l.days_late) AS max_days_late
  FROM po_lines AS l
  GROUP BY
    l.po_id
),

received AS (
  SELECT
    p.supplier_code,
    p.ordered_at,
    DATEDIFF('hour', p.ordered_at, p.received_at) / 24 AS lead_time_days,
    f.po_fill_rate,
    f.max_days_late
  FROM purchase_orders AS p
  INNER JOIN po_fill AS f
    ON p.po_id = f.po_id
  WHERE p.received_at IS NOT NULL
),

final AS (
  SELECT
    r.supplier_code,
    s.supplier_name,
    COUNT(*)::INT AS received_po_count,
    ROUND(
      PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY r.lead_time_days),
      2
    )::NUMBER(10, 2) AS lead_time_p50_days,
    ROUND(
      PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY r.lead_time_days),
      2
    )::NUMBER(10, 2) AS lead_time_p90_days,
    ROUND(STDDEV_SAMP(r.lead_time_days), 2)::NUMBER(10, 2) AS lead_time_stddev_days,
    ROUND(COUNT_IF(r.max_days_late = 0) / COUNT(*), 4)::NUMBER(9, 4) AS on_time_rate,
    ROUND(AVG(r.po_fill_rate), 4)::NUMBER(9, 4) AS avg_fill_rate,
    MAX(r.ordered_at) AS latest_po_at
  FROM received AS r
  LEFT JOIN suppliers AS s
    ON r.supplier_code = s.supplier_code
  GROUP BY
    r.supplier_code,
    s.supplier_name
)

SELECT *
FROM final
