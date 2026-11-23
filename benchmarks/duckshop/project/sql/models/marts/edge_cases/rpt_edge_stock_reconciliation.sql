MODEL (
  description """
    One row per SKU and warehouse that is allocated or counted, reconciling the two with a full
    outer join on shared key columns where either side can be missing
  """,
  tags [edge_case],
  columns (
    sku (type VARCHAR),
    warehouse_code (type VARCHAR),
    allocated_units (type NUMBER(38, 0)),
    counted_units (type NUMBER(38, 0)),
    variance_units (type NUMBER(38, 0), audits [not_null]),
  ),
);

WITH lines AS (
  SELECT
    e.sku,
    e.warehouse_code,
    e.quantity,
    e.status
  FROM __ref("stg_edge_ledger_entries") AS e
),

allocated AS (
  SELECT
    l.sku,
    l.warehouse_code,
    SUM(l.quantity) AS allocated_units
  FROM lines AS l
  WHERE UPPER(l.status) = 'ALLOCATED'
  GROUP BY
    l.sku,
    l.warehouse_code
),

counted AS (
  SELECT
    l.sku,
    l.warehouse_code,
    SUM(l.quantity) AS counted_units
  FROM lines AS l
  WHERE UPPER(l.status) = 'COUNTED'
  GROUP BY
    l.sku,
    l.warehouse_code
),

final AS (
  SELECT
    sku,
    warehouse_code,
    a.allocated_units::NUMBER(38, 0) AS allocated_units,
    c.counted_units::NUMBER(38, 0) AS counted_units,
    (
      COALESCE(c.counted_units, 0) - COALESCE(a.allocated_units, 0)
    )::NUMBER(38, 0) AS variance_units
  FROM allocated AS a
  FULL OUTER JOIN counted AS c
    USING (sku, warehouse_code)
)

SELECT *
FROM final
