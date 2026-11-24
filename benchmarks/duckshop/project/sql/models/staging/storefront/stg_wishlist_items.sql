MODEL (
  description "One row per SKU saved on a wishlist, flattened from the wishlist items array",
  columns (
    wishlist_item_key (type VARCHAR, nullable false, audits [not_null, unique]),
    wishlist_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    sku (type VARCHAR, audits [not_null]),
    added_at (type TIMESTAMP_TZ),
    item_position (type INT),
  ),
);

WITH wishlists AS (
  SELECT *
  FROM __ref("stg_wishlists")
),

items AS (
  SELECT
    w.wishlist_id,
    w.customer_id,
    f.index::INT AS item_position,
    f.value:sku::VARCHAR AS raw_sku,
    TRY_TO_TIMESTAMP_TZ(f.value:added_at::VARCHAR) AS added_at
  FROM wishlists AS w, LATERAL FLATTEN(input => w.items) AS f
  WHERE IS_OBJECT(f.value)
),

final AS (
  SELECT
    (
      i.wishlist_id || ':' || i.item_position::VARCHAR
    )::VARCHAR AS wishlist_item_key,
    i.wishlist_id,
    i.customer_id,
    @sql.normalise_sku(i.raw_sku) AS sku,
    i.added_at,
    i.item_position
  FROM items AS i
  WHERE i.raw_sku IS NOT NULL
)

SELECT *
FROM final
