MODEL (
  description """
    One row per order line with the list price that applied when the order was placed, the markdown
    against list, and product attributes for downstream margin and mix analysis.
  """,
  columns (
    order_line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR),
    sku_family (type VARCHAR),
    product_id (type VARCHAR),
    quantity (type INT),
    unit_amount_minor (type INT),
    list_amount_minor (type INT),
    markdown_pct (type NUMBER(9, 4)),
    gross_amount_minor (type INT),
    net_amount_minor (type INT),
    discount_amount_minor (type INT),
    tax_amount_minor (type INT),
    line_weight_kg (type NUMBER(12, 3)),
    has_list_price (type BOOLEAN),
  ),
);

WITH lines AS (
  SELECT *
  FROM __ref("int_order_lines_discounted")
),

raw_lines AS (
  SELECT *
  FROM __ref("stg_order_lines")
),

orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

price_lists AS (
  SELECT *
  FROM __ref("stg_price_lists")
),

variants AS (
  SELECT *
  FROM __ref("stg_product_variants")
),

line_context AS (
  SELECT
    l.order_line_id,
    l.order_id,
    l.sku,
    l.quantity,
    r.unit_amount_minor,
    l.gross_amount_minor,
    l.net_amount_minor,
    l.discount_amount_minor,
    l.tax_amount_minor,
    o.currency_code,
    o.placed_at,
    o.sales_channel
  FROM lines AS l
  INNER JOIN raw_lines AS r
    ON l.order_line_id = r.order_line_id
  INNER JOIN orders AS o
    ON l.order_id = o.order_id
),

candidate_prices AS (
  SELECT
    c.order_line_id,
    p.list_amount_minor,
    p.valid_from
  FROM line_context AS c
  INNER JOIN price_lists AS p
    ON c.sku = p.sku
    AND c.currency_code = p.currency_code
    AND c.placed_at >= p.valid_from
    AND (
      c.placed_at < p.valid_to
      OR p.valid_to IS NULL
    )
  WHERE
    c.quantity >= p.min_quantity
    AND (
      ARRAY_SIZE(p.channel_restrictions) = 0
      OR ARRAY_CONTAINS(c.sales_channel::VARIANT, p.channel_restrictions)
    )
),

applied_prices AS (
  SELECT
    cp.order_line_id,
    MAX_BY(cp.list_amount_minor, cp.valid_from) AS list_amount_minor
  FROM candidate_prices AS cp
  GROUP BY
    cp.order_line_id
),

final AS (
  SELECT
    c.order_line_id,
    c.order_id,
    c.sku,
    __udf("udf__sku_family")(c.sku)::VARCHAR AS sku_family,
    v.product_id,
    c.quantity,
    c.unit_amount_minor,
    a.list_amount_minor::INT AS list_amount_minor,
    ROUND(100
    * (
      1 - c.unit_amount_minor / NULLIF(a.list_amount_minor, 0)
    ), 4)::NUMBER(9, 4) AS markdown_pct,
    c.gross_amount_minor,
    c.net_amount_minor,
    c.discount_amount_minor,
    c.tax_amount_minor,
    (
      c.quantity * COALESCE(v.weight_kg, 0)
    )::NUMBER(12, 3) AS line_weight_kg,
    (
      a.list_amount_minor IS NOT NULL
    ) AS has_list_price
  FROM line_context AS c
  LEFT JOIN applied_prices AS a
    ON c.order_line_id = a.order_line_id
  LEFT JOIN variants AS v
    ON c.sku = v.sku
)

SELECT *
FROM final
