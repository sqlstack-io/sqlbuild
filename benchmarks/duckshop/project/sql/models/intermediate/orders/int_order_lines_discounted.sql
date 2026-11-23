MODEL (
  description """
    One row per order line with its discounts rolled up by kind, the promotions behind them, and the
    net amount after discounts in minor units.
  """,
  columns (
    order_line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    quantity (type INT),
    gross_amount_minor (type INT),
    discount_amount_minor (type INT),
    net_amount_minor (type INT),
    tax_amount_minor (type INT),
    discount_by_kind (type OBJECT),
    promo_codes (type ARRAY),
    has_stacked_promotions (type BOOLEAN),
    discount_share (type NUMBER(9, 6)),
  ),
);

WITH order_lines AS (
  SELECT *
  FROM __ref("stg_order_lines")
),

line_discounts AS (
  SELECT *
  FROM __ref("stg_order_line_discounts")
),

promotions AS (
  SELECT *
  FROM __ref("stg_promotions")
),

discounts_with_promotion AS (
  SELECT
    d.order_line_id,
    d.discount_kind,
    COALESCE(d.discount_amount_minor, 0) AS discount_amount_minor,
    p.promo_code,
    COALESCE(p.is_stackable, FALSE) AS is_stackable
  FROM line_discounts AS d
  LEFT JOIN promotions AS p
    ON d.discount_code = p.promo_code
),

kind_totals AS (
  SELECT
    dp.order_line_id,
    dp.discount_kind,
    SUM(dp.discount_amount_minor) AS kind_amount_minor
  FROM discounts_with_promotion AS dp
  GROUP BY
    dp.order_line_id,
    dp.discount_kind
),

kind_objects AS (
  SELECT
    k.order_line_id,
    OBJECT_AGG(k.discount_kind, k.kind_amount_minor::VARIANT) AS discount_by_kind,
    SUM(k.kind_amount_minor) AS discount_amount_minor
  FROM kind_totals AS k
  GROUP BY
    k.order_line_id
),

promotion_sets AS (
  SELECT
    dp.order_line_id,
    ARRAY_AGG(DISTINCT dp.promo_code) WITHIN GROUP (ORDER BY dp.promo_code) AS promo_codes,
    COUNT(DISTINCT dp.promo_code) AS promotion_count,
    BOOLAND_AGG(dp.is_stackable) AS all_stackable
  FROM discounts_with_promotion AS dp
  WHERE dp.promo_code IS NOT NULL
  GROUP BY
    dp.order_line_id
),

final AS (
  SELECT
    l.order_line_id,
    l.order_id,
    l.sku,
    l.quantity,
    l.gross_amount_minor,
    LEAST(COALESCE(k.discount_amount_minor, 0), l.gross_amount_minor)::INT AS discount_amount_minor,
    (
      l.gross_amount_minor - LEAST(COALESCE(k.discount_amount_minor, 0), l.gross_amount_minor)
    )::INT AS net_amount_minor,
    l.tax_amount_minor,
    COALESCE(k.discount_by_kind, OBJECT_CONSTRUCT())::OBJECT AS discount_by_kind,
    COALESCE(ps.promo_codes, ARRAY_CONSTRUCT())::ARRAY AS promo_codes,
    (
      COALESCE(ps.promotion_count, 0) > 1
      AND COALESCE(ps.all_stackable, FALSE)
    ) AS has_stacked_promotions,
    ROUND(
      COALESCE(k.discount_amount_minor, 0) / NULLIF(l.gross_amount_minor, 0),
      6
    )::NUMBER(9, 6) AS discount_share
  FROM order_lines AS l
  LEFT JOIN kind_objects AS k
    ON l.order_line_id = k.order_line_id
  LEFT JOIN promotion_sets AS ps
    ON l.order_line_id = ps.order_line_id
)

SELECT *
FROM final
