MODEL (
  description "One row per live order line with canonical SKU and gross amounts in minor units",
  columns (
    order_line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    line_no (type INT, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    quantity (type INT),
    unit_amount_minor (type INT),
    gross_amount_minor (type INT),
    tax_amount_minor (type INT),
    discount_count (type INT),
    discounts (type ARRAY),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__order_lines")
),

@sql.cdc_latest(src, src.key := (order_line_id))

final AS (
  SELECT
    l.order_line_id,
    l.order_id,
    l.line_no,
    @sql.normalise_sku(l.sku) AS sku,
    GREATEST(l.quantity, 0)::INT AS quantity,
    l.unit_amount_minor,
    (
      GREATEST(l.quantity, 0) * l.unit_amount_minor
    )::INT AS gross_amount_minor,
    COALESCE(l.tax_amount_minor, 0)::INT AS tax_amount_minor,
    IFF(IS_ARRAY(l.discounts), ARRAY_SIZE(l.discounts), 0)::INT AS discount_count,
    IFF(IS_ARRAY(l.discounts), l.discounts, ARRAY_CONSTRUCT())::ARRAY AS discounts,
    @sql.load_metadata(l)
  FROM cdc_latest AS l
)
SELECT *
FROM final
