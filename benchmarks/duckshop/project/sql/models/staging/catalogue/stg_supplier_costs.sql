MODEL (
  description "One row per live supplier unit cost for a SKU from an effective date",
  columns (
    cost_id (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_code (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    unit_cost_minor (type INT),
    effective_from (type DATE),
    effective_to (type DATE),
    volume_break_count (type INT),
    best_break_cost_minor (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__supplier_costs")
),

@sql.cdc_latest(src, src.key := (cost_id))

final AS (
  SELECT
    c.cost_id,
    UPPER(c.supplier_code)::VARCHAR AS supplier_code,
    @sql.normalise_sku(c.sku) AS sku,
    UPPER(c.currency_code)::VARCHAR AS currency_code,
    c.unit_cost_minor::INT AS unit_cost_minor,
    c.effective_from,
    DATEADD(
      'day',
      -1,
      LEAD(c.effective_from) OVER (
        PARTITION BY c.supplier_code, @sql.normalise_sku(c.sku)
        ORDER BY c.effective_from
      )
    )::DATE AS effective_to,
    ARRAY_SIZE(COALESCE(c.volume_breaks, ARRAY_CONSTRUCT()))::INT AS volume_break_count,
    ARRAY_MIN(
      TRANSFORM(c.volume_breaks, b VARIANT -> b:unit_cost_minor)
    )::INT AS best_break_cost_minor,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
