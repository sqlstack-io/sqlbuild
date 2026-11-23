MODEL (
  description """
    One row per top-level category and currency with SKU counts, average margin and below-cost SKUs
  """,
  columns (
    margin_key (type VARCHAR, nullable false, audits [not_null, unique]),
    category_level_1 (type VARCHAR),
    currency_code (type VARCHAR),
    sku_count (type INT),
    average_margin_pct (type NUMBER(9, 2)),
    p25_margin_pct (type NUMBER(9, 2)),
    p75_margin_pct (type NUMBER(9, 2)),
    below_cost_skus (type INT),
  ),
);

WITH margins AS (
  SELECT *
  FROM __ref("int_product_margin")
),

final AS (
  SELECT
    (
      COALESCE(m.category_level_1, 'uncategorised') || ':' || m.currency_code
    )::VARCHAR AS margin_key,
    COALESCE(m.category_level_1, 'uncategorised')::VARCHAR AS category_level_1,
    m.currency_code,
    COUNT(*)::INT AS sku_count,
    ROUND(AVG(m.margin_pct), 2)::NUMBER(9, 2) AS average_margin_pct,
    ROUND(
      PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY m.margin_pct),
      2
    )::NUMBER(9, 2) AS p25_margin_pct,
    ROUND(
      PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY m.margin_pct),
      2
    )::NUMBER(9, 2) AS p75_margin_pct,
    COUNT_IF(m.is_below_cost)::INT AS below_cost_skus
  FROM margins AS m
  GROUP BY
    COALESCE(m.category_level_1, 'uncategorised'),
    m.currency_code
)

SELECT *
FROM final
