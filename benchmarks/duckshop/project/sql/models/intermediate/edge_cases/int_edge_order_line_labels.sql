MODEL (
  description """
    One row per order with its lines listed in a total order, the priciest and cheapest SKU, the
    customer tier taken from any line, and percentiles, medians and modes of its unit prices
  """,
  tags [edge_case],
  columns (
    order_ref (type VARCHAR, nullable false, audits [not_null, unique]),
    line_count (type INT),
    sku_list (type VARCHAR),
    sku_array (type ARRAY),
    priciest_sku (type VARCHAR),
    cheapest_sku (type VARCHAR),
    customer_tier (type VARCHAR),
    median_unit_price (type NUMBER(38, 3)),
    percentile_unit_price (type NUMBER(38, 3)),
    percentile_disc_unit_price (type NUMBER(38, 0)),
    modal_quantity (type NUMBER(38, 0)),
  ),
);

WITH lines AS (
  SELECT
    e.entry_id,
    e.order_ref,
    e.line_no,
    e.sku,
    e.quantity,
    e.unit_price_minor,
    e.customer_tier
  FROM __ref("stg_edge_ledger_entries") AS e
  WHERE e.order_ref IS NOT NULL
),

final AS (
  SELECT
    l.order_ref,
    COUNT(*)::INT AS line_count,
    LISTAGG(l.sku, ',') WITHIN GROUP (ORDER BY l.line_no, l.sku, l.entry_id)::VARCHAR AS sku_list,
    ARRAY_AGG(l.sku) WITHIN GROUP (ORDER BY l.line_no, l.sku, l.entry_id)::ARRAY AS sku_array,
    MAX_BY(l.sku, l.unit_price_minor)::VARCHAR AS priciest_sku,
    MIN_BY(l.sku, l.unit_price_minor)::VARCHAR AS cheapest_sku,
    ANY_VALUE(l.customer_tier)::VARCHAR AS customer_tier,
    MEDIAN(l.unit_price_minor)::NUMBER(38, 3) AS median_unit_price,
    PERCENTILE_CONT(0.25) WITHIN GROUP (
      ORDER BY l.unit_price_minor
    )::NUMBER(38, 3) AS percentile_unit_price,
    PERCENTILE_DISC(0.25) WITHIN GROUP (
      ORDER BY l.unit_price_minor
    )::NUMBER(38, 0) AS percentile_disc_unit_price,
    MODE(l.quantity)::NUMBER(38, 0) AS modal_quantity
  FROM lines AS l
  GROUP BY
    l.order_ref
)

SELECT *
FROM final
