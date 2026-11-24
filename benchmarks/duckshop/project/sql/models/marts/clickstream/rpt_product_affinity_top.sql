MODEL (
  description """
    One row per SKU with its five most co-viewed SKUs and their lift, as an ordered array
  """,
  columns (
    sku (type VARCHAR, nullable false, audits [not_null, unique]),
    title (type VARCHAR),
    sku_viewers (type INT),
    top_co_viewed (type ARRAY),
    best_lift (type NUMBER(12, 4)),
  ),
);

WITH affinity AS (
  SELECT *
  FROM __ref("int_product_view_affinity")
),

products AS (
  SELECT *
  FROM __ref("dim_products")
),

top_pairs AS (
  SELECT
    a.sku,
    a.co_viewed_sku,
    a.lift,
    a.pair_rank,
    a.sku_viewers
  FROM affinity AS a
  WHERE a.pair_rank <= 5
),

final AS (
  SELECT
    t.sku,
    MAX(p.title)::VARCHAR AS title,
    MAX(t.sku_viewers)::INT AS sku_viewers,
    ARRAY_AGG(
      OBJECT_CONSTRUCT('sku', t.co_viewed_sku, 'lift', t.lift)
    ) WITHIN GROUP (ORDER BY t.pair_rank, t.co_viewed_sku)::ARRAY AS top_co_viewed,
    MAX(t.lift)::NUMBER(12, 4) AS best_lift
  FROM top_pairs AS t
  LEFT JOIN products AS p
    ON t.sku = p.sku
  GROUP BY
    t.sku
)

SELECT *
FROM final
