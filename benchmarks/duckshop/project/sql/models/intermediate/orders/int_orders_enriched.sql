MODEL (
  description """
    One row per order with its line totals, payment position, customer, shipping destination,
    storefront region and local business date, ready for currency conversion.
  """,
  enums (
    _basket_size [SINGLE, SMALL, LARGE],
  ),
  constants (
    _large_basket_units 6,
  ),
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    storefront_code (type VARCHAR),
    region_code (type VARCHAR),
    storefront_timezone (type VARCHAR),
    currency_code (type VARCHAR, audits [not_null]),
    placed_at (type TIMESTAMP_TZ),
    placed_date_local (type DATE),
    order_status (type VARCHAR),
    sales_channel (type VARCHAR),
    line_count (type INT),
    units (type INT),
    gross_amount_minor (type INT),
    discount_amount_minor (type INT),
    net_amount_minor (type INT),
    tax_amount_minor (type INT),
    captured_amount_minor (type INT),
    outstanding_amount_minor (type INT),
    promo_code (type VARCHAR),
    ship_to_country_code (type VARCHAR),
    is_cross_border (type BOOLEAN),
    is_first_party_customer (type BOOLEAN),
    basket_size (type VARCHAR),
    is_gift (type BOOLEAN),
    utm_source (type VARCHAR),
    utm_medium (type VARCHAR),
    utm_campaign (type VARCHAR),
    total_weight_kg (type NUMBER(12, 3)),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

lines AS (
  SELECT *
  FROM __ref("int_order_lines_priced")
),

customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

payments AS (
  SELECT *
  FROM __ref("stg_payments")
),

addresses AS (
  SELECT *
  FROM __ref("stg_customer_addresses")
),

redemptions AS (
  SELECT *
  FROM __ref("stg_promotion_redemptions")
),

countries AS (
  SELECT *
  FROM __seed("countries")
),

line_totals AS (
  SELECT
    l.order_id,
    COUNT(*) AS line_count,
    SUM(l.quantity) AS units,
    SUM(l.gross_amount_minor) AS gross_amount_minor,
    SUM(l.discount_amount_minor) AS discount_amount_minor,
    SUM(l.net_amount_minor) AS net_amount_minor,
    SUM(l.tax_amount_minor) AS tax_amount_minor,
    SUM(l.line_weight_kg) AS total_weight_kg
  FROM lines AS l
  GROUP BY
    l.order_id
),

captured AS (
  SELECT
    p.order_id,
    SUM(
      IFF(p.gateway_status IN ('captured', 'settled'), p.amount_minor, 0)
    ) AS captured_amount_minor
  FROM payments AS p
  GROUP BY
    p.order_id
),

first_redemption AS (
  SELECT
    r.order_id,
    r.promo_code
  FROM redemptions AS r
  QUALIFY ROW_NUMBER() OVER (PARTITION BY r.order_id ORDER BY r.redeemed_at, r.redemption_id) = 1
),

shipping AS (
  SELECT
    o.order_id,
    a.country_code AS ship_to_country_code
  FROM orders AS o
  INNER JOIN addresses AS a
    ON o.shipping_address_id = a.address_id
),

storefronts AS (
  SELECT
    c.country_code AS storefront_code,
    c.region_code,
    c.default_timezone
  FROM countries AS c
),

final AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.storefront_code,
    COALESCE(s.region_code, cu.region_code)::VARCHAR AS region_code,
    COALESCE(s.default_timezone, 'UTC')::VARCHAR AS storefront_timezone,
    o.currency_code,
    o.placed_at,
    @sql.local_business_date(o.placed_at, COALESCE(s.default_timezone, 'UTC')) AS placed_date_local,
    o.order_status,
    COALESCE(o.sales_channel, @enum("sales_channel").WEB)::VARCHAR AS sales_channel,
    COALESCE(t.line_count, 0)::INT AS line_count,
    COALESCE(t.units, 0)::INT AS units,
    COALESCE(t.gross_amount_minor, 0)::INT AS gross_amount_minor,
    COALESCE(t.discount_amount_minor, 0)::INT AS discount_amount_minor,
    COALESCE(t.net_amount_minor, 0)::INT AS net_amount_minor,
    COALESCE(t.tax_amount_minor, 0)::INT AS tax_amount_minor,
    COALESCE(c.captured_amount_minor, 0)::INT AS captured_amount_minor,
    GREATEST(
      COALESCE(t.net_amount_minor, 0)
        + COALESCE(t.tax_amount_minor, 0)
        - COALESCE(c.captured_amount_minor, 0),
      0
    )::INT AS outstanding_amount_minor,
    r.promo_code,
    sh.ship_to_country_code,
    COALESCE(sh.ship_to_country_code <> o.storefront_code, FALSE) AS is_cross_border,
    NOT RLIKE(
      COALESCE(cu.email_normalised, ''),
      '.*@example\\.(org|net)$'
    ) AS is_first_party_customer,
    CASE
      WHEN COALESCE(t.units, 0) <= 1
      THEN @enum("_basket_size").SINGLE
      WHEN t.units < @const("_large_basket_units")
      THEN @enum("_basket_size").SMALL
      ELSE @enum("_basket_size").LARGE
    END::VARCHAR AS basket_size,
    COALESCE(o.is_gift, FALSE) AS is_gift,
    NULLIF(LOWER(o.utm_source), '')::VARCHAR AS utm_source,
    NULLIF(LOWER(o.utm_medium), '')::VARCHAR AS utm_medium,
    NULLIF(LOWER(o.utm_campaign), '')::VARCHAR AS utm_campaign,
    COALESCE(t.total_weight_kg, 0)::NUMBER(12, 3) AS total_weight_kg
  FROM orders AS o
  LEFT JOIN line_totals AS t
    ON o.order_id = t.order_id
  LEFT JOIN captured AS c
    ON o.order_id = c.order_id
  LEFT JOIN first_redemption AS r
    ON o.order_id = r.order_id
  LEFT JOIN shipping AS sh
    ON o.order_id = sh.order_id
  LEFT JOIN storefronts AS s
    ON o.storefront_code = s.storefront_code
  LEFT JOIN customers AS cu
    ON o.customer_id = cu.customer_id
)

SELECT *
FROM final
