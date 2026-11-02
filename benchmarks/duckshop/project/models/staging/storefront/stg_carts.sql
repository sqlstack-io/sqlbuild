MODEL (
  description "One row per live shopping cart",
  columns (
    cart_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR),
    anonymous_id (type VARCHAR),
    storefront_code (type VARCHAR),
    cart_status (type VARCHAR),
    created_at (type TIMESTAMP_TZ),
    updated_at (type TIMESTAMP_TZ),
    line_count (type INT),
    coupon_code (type VARCHAR),
    contents (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__carts")
),

@sql.cdc_latest(src, src.key := (cart_id))

final AS (
  SELECT
    c.cart_id,
    c.customer_id,
    LOWER(c.anonymous_id)::VARCHAR AS anonymous_id,
    UPPER(c.storefront_code)::VARCHAR AS storefront_code,
    LOWER(c.cart_status)::VARCHAR AS cart_status,
    c.created_at,
    c.updated_at,
    ARRAY_SIZE(COALESCE(c.contents:items, ARRAY_CONSTRUCT()))::INT AS line_count,
    UPPER(@sql.json_text(c.contents:coupon))::VARCHAR AS coupon_code,
    c.contents::OBJECT AS contents,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
