MODEL (
  description "One row per live wishlist",
  columns (
    wishlist_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    created_at (type TIMESTAMP_TZ),
    is_public (type BOOLEAN),
    item_count (type INT),
    items (type ARRAY),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__wishlists")
),

@sql.cdc_latest(src, src.key := (wishlist_id))

final AS (
  SELECT
    w.wishlist_id,
    w.customer_id,
    w.created_at,
    COALESCE(w.is_public, FALSE) AS is_public,
    ARRAY_SIZE(COALESCE(w.items, ARRAY_CONSTRUCT()))::INT AS item_count,
    w.items::ARRAY AS items,
    @sql.load_metadata(w)
  FROM cdc_latest AS w
)
SELECT *
FROM final
