MODEL (
  description """
    One row per partner listing with its normalised state, the catalogue product it sells, our
    current list price and available stock for the SKU, recent partner sales, and a health flag for
    listings that are live but cannot be fulfilled.
  """,
  columns (
    listing_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    sku (type VARCHAR),
    listing_state (type VARCHAR, audits [
      accepted_values (values ["live", "paused", "rejected", "draft", "ended", "unknown"]),
    ]),
    listed_at (type TIMESTAMP_TZ),
    days_listed (type INT),
    product_title (type VARCHAR),
    brand_name (type VARCHAR),
    is_catalogue_active (type BOOLEAN),
    current_list_price_minor (type INT),
    list_price_currency_code (type VARCHAR),
    available_to_promise (type INT),
    commission_pct (type NUMBER(5, 2)),
    fulfilled_by_partner (type BOOLEAN),
    fulfilment_method (type fulfilment_method),
    term_keys (type ARRAY),
    units_sold_30d (type INT),
    partners_listing_sku (type INT),
    listing_health (type VARCHAR),
  ),
);

WITH listings AS (
  SELECT *
  FROM __ref("stg_partner_listings")
),

catalogue AS (
  SELECT *
  FROM __ref("int_product_catalogue")
),

reservations AS (
  SELECT *
  FROM __ref("int_reservations_open")
),

price_history AS (
  SELECT *
  FROM __ref("int_price_history")
),

partner_orders AS (
  SELECT *
  FROM __ref("int_partner_orders_unified")
),

order_lines AS (
  SELECT *
  FROM __ref("stg_partner_order_lines")
),

normalised AS (
  SELECT
    l.listing_id,
    l.partner_code,
    l.sku,
    CASE
      WHEN REGEXP_LIKE(l.listing_status, '^(live|active|published|on_?sale)$')
      THEN 'live'
      WHEN REGEXP_LIKE(l.listing_status, '^(paused|inactive|suspended|out_?of_?stock)$')
      THEN 'paused'
      WHEN REGEXP_LIKE(l.listing_status, '^(rejected|blocked|error.*)$')
      THEN 'rejected'
      WHEN REGEXP_LIKE(l.listing_status, '^(draft|pending.*|in_?review)$')
      THEN 'draft'
      WHEN REGEXP_LIKE(l.listing_status, '^(ended|closed|deleted|archived)$')
      THEN 'ended'
      ELSE 'unknown'
    END AS listing_state,
    l.listed_at,
    l.commission_pct,
    l.fulfilled_by_partner,
    OBJECT_KEYS(COALESCE(l.terms, OBJECT_CONSTRUCT())) AS term_keys,
    COUNT(*) OVER (PARTITION BY l.sku) AS partners_listing_sku
  FROM listings AS l
),

current_prices AS (
  SELECT
    p.sku,
    p.currency_code,
    p.list_amount_minor
  FROM price_history AS p
  WHERE
    p.effective_valid_to IS NULL
    OR p.effective_valid_to > '@@valuation_date'::DATE
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY p.sku
      ORDER BY IFF(
        p.currency_code = @const("default_currency"),
        0,
        1
      ), p.valid_from DESC, p.price_list_id
    ) = 1
),

stock AS (
  SELECT
    r.sku,
    SUM(GREATEST(r.available_to_promise, 0)) AS available_to_promise
  FROM reservations AS r
  GROUP BY
    r.sku
),

recent_orders AS (
  SELECT
    o.partner_order_key,
    o.partner_code
  FROM partner_orders AS o
  WHERE
    o.placed_date > DATEADD(
      'day',
      -30,
      '@@valuation_date'::DATE
    )
),

recent_sales AS (
  SELECT
    o.partner_code,
    l.sku,
    SUM(l.quantity) AS units_sold_30d
  FROM order_lines AS l
  INNER JOIN recent_orders AS o
    ON l.partner_order_key = o.partner_order_key
  GROUP BY
    o.partner_code,
    l.sku
),

joined AS (
  SELECT
    n.listing_id,
    n.partner_code,
    n.sku,
    n.listing_state,
    n.listed_at,
    c.title AS product_title,
    c.brand_name,
    c.is_active AS is_catalogue_active,
    p.list_amount_minor AS current_list_price_minor,
    p.currency_code AS list_price_currency_code,
    COALESCE(s.available_to_promise, 0) AS available_to_promise,
    n.commission_pct,
    n.fulfilled_by_partner,
    n.term_keys,
    COALESCE(rs.units_sold_30d, 0) AS units_sold_30d,
    n.partners_listing_sku
  FROM normalised AS n
  LEFT JOIN catalogue AS c
    ON n.sku = c.sku
  LEFT JOIN current_prices AS p
    ON n.sku = p.sku
  LEFT JOIN stock AS s
    ON n.sku = s.sku
  LEFT JOIN recent_sales AS rs
    ON n.partner_code = rs.partner_code
    AND n.sku = rs.sku
),

final AS (
  SELECT
    j.listing_id,
    j.partner_code,
    j.sku,
    j.listing_state::VARCHAR AS listing_state,
    j.listed_at,
    DATEDIFF(
      'day',
      j.listed_at,
      '@@valuation_date'::DATE
    )::INT AS days_listed,
    j.product_title::VARCHAR AS product_title,
    j.brand_name::VARCHAR AS brand_name,
    j.is_catalogue_active::BOOLEAN AS is_catalogue_active,
    j.current_list_price_minor::INT AS current_list_price_minor,
    j.list_price_currency_code::VARCHAR AS list_price_currency_code,
    j.available_to_promise::INT AS available_to_promise,
    j.commission_pct,
    j.fulfilled_by_partner,
    IFF(
      j.fulfilled_by_partner,
      @enum("fulfilment_method").PARTNER,
      @enum("fulfilment_method").HOME_DELIVERY
    ) AS fulfilment_method,
    j.term_keys::ARRAY AS term_keys,
    j.units_sold_30d::INT AS units_sold_30d,
    j.partners_listing_sku::INT AS partners_listing_sku,
    CASE
      WHEN j.listing_state <> 'live'
      THEN 'not_live'
      WHEN j.product_title IS NULL
      THEN 'unknown_sku'
      WHEN NOT COALESCE(j.is_catalogue_active, FALSE)
      THEN 'discontinued'
      WHEN NOT j.fulfilled_by_partner
      AND j.available_to_promise <= 0
      THEN 'unfulfillable'
      WHEN j.current_list_price_minor IS NULL
      THEN 'unpriced'
      ELSE 'healthy'
    END::VARCHAR AS listing_health
  FROM joined AS j
)

SELECT *
FROM final
