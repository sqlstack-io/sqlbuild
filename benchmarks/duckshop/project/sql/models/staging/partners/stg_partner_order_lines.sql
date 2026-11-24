MODEL (
  description """
    One row per partner order line from both payload versions, with amounts in minor units
  """,
  columns (
    partner_order_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_order_key (type VARCHAR, audits [not_null]),
    payload_version (type INT),
    sku (type VARCHAR),
    quantity (type INT),
    unit_amount_minor (type NUMBER(38, 0)),
  ),
);

WITH orders_v1 AS (
  SELECT *
  FROM __ref("stg_partner_orders_v1")
),

orders_v2 AS (
  SELECT *
  FROM __ref("stg_partner_orders_v2")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

orders_v1_exponent AS (
  SELECT
    o.partner_order_key,
    o.lines,
    c.minor_unit_exponent
  FROM orders_v1 AS o
  INNER JOIN currencies AS c
    ON o.currency_code = c.currency_code
),

lines_v1 AS (
  SELECT
    o.partner_order_key,
    1 AS payload_version,
    l.index AS line_index,
    l.value:sku::VARCHAR AS raw_sku,
    l.value:qty::INT AS quantity,
    ROUND(TO_DECIMAL(l.value:price::VARCHAR, 18, 3) * POWER(10, o.minor_unit_exponent)
    )::NUMBER(38, 0) AS unit_amount_minor
  FROM orders_v1_exponent AS o, LATERAL FLATTEN(input => o.lines) AS l
),

lines_v2 AS (
  SELECT
    o.partner_order_key,
    2 AS payload_version,
    i.index AS line_index,
    i.value:seller_sku::VARCHAR AS raw_sku,
    i.value:quantity::INT AS quantity,
    i.value:unit_price_minor::NUMBER(38, 0) AS unit_amount_minor
  FROM orders_v2 AS o, LATERAL FLATTEN(input => o.items) AS i
),

all_lines AS (
  SELECT
    v1.partner_order_key,
    v1.payload_version,
    v1.line_index,
    v1.raw_sku,
    v1.quantity,
    v1.unit_amount_minor
  FROM lines_v1 AS v1
  UNION ALL
  SELECT
    v2.partner_order_key,
    v2.payload_version,
    v2.line_index,
    v2.raw_sku,
    v2.quantity,
    v2.unit_amount_minor
  FROM lines_v2 AS v2
),

final AS (
  SELECT
    (
      a.partner_order_key || '#' || a.line_index::VARCHAR
    )::VARCHAR AS partner_order_line_key,
    a.partner_order_key,
    a.payload_version::INT AS payload_version,
    @sql.normalise_sku(a.raw_sku) AS sku,
    a.quantity::INT AS quantity,
    a.unit_amount_minor
  FROM all_lines AS a
)

SELECT *
FROM final
