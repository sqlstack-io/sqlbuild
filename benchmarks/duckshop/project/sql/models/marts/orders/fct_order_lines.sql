MODEL (
  description """
    One row per order line with list, discount and net amounts in minor units, the promotions
    applied, and the product's family and category.
  """,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    order_line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null, relationships (to __ref("fct_orders"), field order_id)]),
    sku (type VARCHAR, audits [not_null]),
    sku_family (type VARCHAR),
    category_level_1 (type VARCHAR),
    quantity (type INT, audits [non_negative]),
    unit_amount_minor (type INT),
    list_amount_minor (type INT),
    markdown_pct (type NUMBER(9, 4)),
    gross_amount_minor (type INT),
    discount_amount_minor (type INT),
    net_amount_minor (type INT),
    tax_amount_minor (type INT),
    promo_codes (type ARRAY),
    has_stacked_promotions (type BOOLEAN),
    line_weight_kg (type NUMBER(12, 3)),
  ),
);

WITH priced AS (
  SELECT *
  FROM __ref("int_order_lines_priced")
),

discounted AS (
  SELECT *
  FROM __ref("int_order_lines_discounted")
),

catalogue AS (
  SELECT *
  FROM __ref("int_product_catalogue")
),

final AS (
  SELECT
    p.order_line_id,
    p.order_id,
    p.sku,
    p.sku_family,
    c.category_level_1,
    p.quantity,
    p.unit_amount_minor,
    p.list_amount_minor,
    p.markdown_pct,
    p.gross_amount_minor,
    p.discount_amount_minor,
    p.net_amount_minor,
    p.tax_amount_minor,
    COALESCE(d.promo_codes, ARRAY_CONSTRUCT())::ARRAY AS promo_codes,
    COALESCE(d.has_stacked_promotions, FALSE)::BOOLEAN AS has_stacked_promotions,
    p.line_weight_kg
  FROM priced AS p
  LEFT JOIN discounted AS d
    ON p.order_line_id = d.order_line_id
  LEFT JOIN catalogue AS c
    ON p.sku = c.sku
)

SELECT *
FROM final
