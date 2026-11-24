MODEL (
  description """
    One row per cart with its value, whether it converted to an order by the same customer within a
    day of its last update, and the hours it sat idle.
  """,
  constants (
    _conversion_window_hours 24,
  ),
  columns (
    cart_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR),
    storefront_code (type VARCHAR),
    cart_status (type VARCHAR),
    item_count (type INT),
    units (type INT),
    cart_value_minor (type INT),
    last_activity_at (type TIMESTAMP_TZ),
    converted_order_id (type VARCHAR),
    is_converted (type BOOLEAN),
    is_abandoned (type BOOLEAN),
    hours_to_convert (type NUMBER(10, 2)),
    distinct_sku_families (type INT),
  ),
);

WITH carts AS (
  SELECT *
  FROM __ref("stg_carts")
),

cart_items AS (
  SELECT *
  FROM __ref("stg_cart_items")
),

orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

item_totals AS (
  SELECT
    i.cart_id,
    COUNT(*) AS item_count,
    SUM(COALESCE(i.quantity, 1)) AS units,
    SUM(COALESCE(i.quantity, 1) * COALESCE(i.unit_amount_minor, 0)) AS cart_value_minor,
    COUNT(DISTINCT __udf("udf__sku_family")(i.sku)) AS distinct_sku_families
  FROM cart_items AS i
  GROUP BY
    i.cart_id
),

cart_activity AS (
  SELECT
    c.cart_id,
    c.customer_id,
    c.storefront_code,
    c.cart_status,
    COALESCE(c.updated_at, c.created_at) AS last_activity_at
  FROM carts AS c
),

customer_orders AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.placed_at
  FROM orders AS o
  WHERE o.customer_id IS NOT NULL
),

matches AS (
  SELECT
    a.cart_id,
    co.order_id,
    co.placed_at
  FROM cart_activity AS a
  INNER JOIN customer_orders AS co
    ON a.customer_id = co.customer_id
    AND co.placed_at >= a.last_activity_at
  WHERE DATEDIFF('hour', a.last_activity_at, co.placed_at) <= @const("_conversion_window_hours")
),

first_match AS (
  SELECT
    m.cart_id,
    MIN_BY(m.order_id, m.placed_at) AS converted_order_id,
    MIN(m.placed_at) AS converted_at
  FROM matches AS m
  GROUP BY
    m.cart_id
),

final AS (
  SELECT
    a.cart_id,
    a.customer_id,
    a.storefront_code,
    a.cart_status,
    COALESCE(t.item_count, 0)::INT AS item_count,
    COALESCE(t.units, 0)::INT AS units,
    COALESCE(t.cart_value_minor, 0)::INT AS cart_value_minor,
    a.last_activity_at,
    f.converted_order_id::VARCHAR AS converted_order_id,
    (
      f.converted_order_id IS NOT NULL
      OR a.cart_status = 'ordered'
    ) AS is_converted,
    (
      f.converted_order_id IS NULL
      AND a.cart_status NOT IN ('ordered', 'merged')
      AND COALESCE(t.item_count, 0) > 0
    ) AS is_abandoned,
    ROUND(
      DATEDIFF('minute', a.last_activity_at, f.converted_at) / 60,
      2
    )::NUMBER(10, 2) AS hours_to_convert,
    COALESCE(t.distinct_sku_families, 0)::INT AS distinct_sku_families
  FROM cart_activity AS a
  LEFT JOIN item_totals AS t
    ON a.cart_id = t.cart_id
  LEFT JOIN first_match AS f
    ON a.cart_id = f.cart_id
)

SELECT *
FROM final
