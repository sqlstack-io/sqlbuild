MODEL (
  description "One row per item in a live cart, flattened from the cart contents",
  columns (
    cart_item_key (type VARCHAR, nullable false, audits [not_null, unique]),
    cart_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR),
    quantity (type INT),
    unit_amount_minor (type INT),
    added_at (type TIMESTAMP_TZ),
  ),
);

WITH carts AS (
  SELECT *
  FROM __ref("stg_carts")
),

items AS (
  SELECT
    c.cart_id,
    i.index::INT AS item_index,
    i.value:sku::VARCHAR AS raw_sku,
    i.value:qty AS raw_quantity,
    i.value:unit_minor AS raw_unit_minor,
    i.value:added_at::VARCHAR AS raw_added_at
  FROM carts AS c, LATERAL FLATTEN(input => c.contents:items) AS i
),

final AS (
  SELECT
    (
      i.cart_id || '/' || i.item_index::VARCHAR
    )::VARCHAR AS cart_item_key,
    i.cart_id,
    @sql.normalise_sku(i.raw_sku) AS sku,
    IFF(
      TYPEOF(i.raw_quantity) IN ('INTEGER', 'DECIMAL'),
      i.raw_quantity::INT,
      TRY_TO_NUMBER(i.raw_quantity::VARCHAR)
    )::INT AS quantity,
    TRY_TO_NUMBER(i.raw_unit_minor::VARCHAR)::INT AS unit_amount_minor,
    TRY_TO_TIMESTAMP_TZ(i.raw_added_at) AS added_at
  FROM items AS i
)

SELECT *
FROM final
