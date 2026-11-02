MODEL (
  description "One row per live storefront order, latest version from the change log",
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    storefront_code (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR, audits [not_null]),
    placed_at (type TIMESTAMP_TZ, audits [not_null]),
    order_status (type VARCHAR),
    sales_channel (type VARCHAR),
    is_gift (type BOOLEAN),
    utm_source (type VARCHAR),
    utm_medium (type VARCHAR),
    utm_campaign (type VARCHAR),
    device_type (type VARCHAR),
    item_count_hint (type INT),
    shipping_address_id (type VARCHAR),
    attributes (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__orders")
),

@sql.cdc_latest(src, src.key := (order_id))

final AS (
  SELECT
    o.order_id,
    o.customer_id,
    UPPER(o.storefront_code)::VARCHAR AS storefront_code,
    UPPER(o.currency_code)::VARCHAR AS currency_code,
    o.placed_at,
    UPPER(o.status)::VARCHAR AS order_status,
    LOWER(o.attributes:channel::VARCHAR)::VARCHAR AS sales_channel,
    @py.variant_fields("o.attributes", {
      "is_gift": "gift.is_gift BOOLEAN",
      "utm_source": "utm.source VARCHAR",
      "utm_medium": "utm.medium VARCHAR",
      "utm_campaign": "utm.campaign VARCHAR",
      "device_type": "device.type VARCHAR",
    }),
    ARRAY_SIZE(o.attributes:line_skus)::INT AS item_count_hint,
    o.shipping_address_id,
    o.attributes::OBJECT AS attributes,
    @sql.load_metadata(o)
  FROM cdc_latest AS o
)
SELECT *
FROM final
