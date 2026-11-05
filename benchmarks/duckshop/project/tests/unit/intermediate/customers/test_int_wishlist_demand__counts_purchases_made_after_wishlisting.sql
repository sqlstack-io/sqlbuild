TEST (name "int_wishlist_demand__counts_purchases_made_after_wishlisting");

WITH __ref__stg_wishlist_items AS (
  SELECT
    'W1#1' AS wishlist_item_key,
    'W1' AS wishlist_id,
    'C1' AS customer_id,
    'DUCK-1' AS sku,
    '2026-03-01 00:00:00 +00:00'::TIMESTAMPTZ AS added_at,
    NULL AS item_position
  UNION ALL
  SELECT
    'W2#1',
    'W2',
    'C1',
    'DUCK-1',
    '2026-03-02 00:00:00 +00:00'::TIMESTAMPTZ,
    NULL
  UNION ALL
  SELECT
    'W3#1',
    'W3',
    'C2',
    'DUCK-1',
    '2026-03-05 00:00:00 +00:00'::TIMESTAMPTZ,
    NULL
),

__ref__stg_wishlists AS (
  SELECT
    'W1' AS wishlist_id,
    'C1' AS customer_id,
    NULL AS created_at,
    TRUE AS is_public,
    NULL AS item_count,
    NULL AS items,
    NULL AS source_seq,
    NULL AS source_loaded_at
  UNION ALL
  SELECT
    'W2',
    'C1',
    NULL,
    FALSE,
    NULL,
    NULL,
    NULL,
    NULL
  UNION ALL
  SELECT
    'W3',
    'C2',
    NULL,
    FALSE,
    NULL,
    NULL,
    NULL,
    NULL
),

__ref__stg_order_lines AS (
  SELECT
    'L1' AS order_line_id,
    'O1' AS order_id,
    NULL AS line_no,
    'DUCK-1' AS sku,
    NULL AS quantity,
    NULL AS unit_amount_minor,
    NULL AS gross_amount_minor,
    NULL AS tax_amount_minor,
    NULL AS discount_count,
    NULL AS discounts,
    NULL AS source_seq,
    NULL AS source_loaded_at
  UNION ALL
  SELECT
    'L2',
    'O2',
    NULL,
    'DUCK-1',
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL
),

__ref__stg_orders AS (
  SELECT
    'O1' AS order_id,
    'C1' AS customer_id,
    NULL AS storefront_code,
    NULL AS currency_code,
    '2026-03-03 12:00:00 +00:00'::TIMESTAMP_TZ AS placed_at,
    NULL AS order_status,
    NULL AS sales_channel,
    NULL AS is_gift,
    NULL AS utm_source,
    NULL AS utm_medium,
    NULL AS utm_campaign,
    NULL AS device_type,
    NULL AS item_count_hint,
    NULL AS shipping_address_id,
    NULL AS attributes,
    NULL AS source_seq,
    NULL AS source_loaded_at
  UNION ALL
  SELECT
    'O2',
    'C2',
    NULL,
    NULL,
    '2026-02-20 12:00:00 +00:00'::TIMESTAMP_TZ,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL
),

__expected__int_wishlist_demand AS (
  SELECT
    'DUCK-1' AS sku,
    2 AS wishlisting_customers,
    1 AS public_wishlist_count,
    1 AS purchased_after_wishlist,
    0.5::DECIMAL(9, 6) AS wishlist_conversion_rate,
    2.5::DECIMAL(10, 2) AS median_days_to_purchase,
    '2026-03-01 00:00:00 +00:00'::TIMESTAMP_TZ AS first_added_at
)
