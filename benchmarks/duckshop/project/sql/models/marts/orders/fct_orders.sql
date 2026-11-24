MODEL (
  description """
    One row per storefront order with its value in order currency and USD, status milestones,
    collected payments and the customer's order sequence.
  """,
  contract enforced,
  model_schema money_with_fx,
  audit_factories [fact_table_quality],
  post_hooks [
    sql("comment_on_relation", relation: "@@CTX:destination.qualified", comment: "DuckShop order facts"),
  ],
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    region_code (type VARCHAR),
    placed_at (type TIMESTAMP_TZ, audits [not_null]),
    placed_date_local (type DATE),
    order_status (type order_status),
    sales_channel (type sales_channel),
    units (type INT),
    gross_amount_usd (type NUMBER(18, 2)),
    discount_amount_usd (type NUMBER(18, 2)),
    tax_amount_usd (type NUMBER(18, 2)),
    promo_code (type VARCHAR),
    utm_source (type VARCHAR),
    utm_medium (type VARCHAR),
    utm_campaign (type VARCHAR),
    basket_size (type VARCHAR),
    order_number (type INT),
    is_first_order (type BOOLEAN),
    first_paid_at (type TIMESTAMP_TZ),
    first_delivered_at (type TIMESTAMP_TZ),
    hours_paid_to_shipped (type NUMBER(12, 2)),
    captured_minor (type INT),
    refunded_minor (type INT),
    net_collected_minor (type INT),
    is_fully_refunded (type BOOLEAN),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

timelines AS (
  SELECT *
  FROM __ref("int_order_status_timeline")
),

payments AS (
  SELECT *
  FROM __ref("int_order_payments_rollup")
),

sequence AS (
  SELECT *
  FROM __ref("int_customer_order_sequence")
),

final AS (
  SELECT
    o.order_id,
    o.customer_id,
    o.region_code,
    o.placed_at,
    o.placed_date_local,
    o.order_status,
    o.sales_channel,
    o.units,
    o.currency_code,
    o.amount,
    o.amount_usd,
    o.rate_to_usd,
    o.rate_date,
    o.gross_amount_usd,
    o.discount_amount_usd,
    o.tax_amount_usd,
    o.promo_code,
    o.utm_source,
    o.utm_medium,
    o.utm_campaign,
    o.basket_size,
    s.order_number,
    COALESCE(s.is_first_order, FALSE)::BOOLEAN AS is_first_order,
    t.first_paid_at,
    t.first_delivered_at,
    t.hours_paid_to_shipped,
    COALESCE(p.captured_minor, 0)::INT AS captured_minor,
    COALESCE(p.refunded_minor, 0)::INT AS refunded_minor,
    COALESCE(p.net_collected_minor, 0)::INT AS net_collected_minor,
    (
      COALESCE(p.captured_minor, 0) > 0
      AND COALESCE(p.refunded_minor, 0) >= p.captured_minor
    )::BOOLEAN AS is_fully_refunded
  FROM orders AS o
  LEFT JOIN timelines AS t
    ON o.order_id = t.order_id
  LEFT JOIN payments AS p
    ON o.order_id = p.order_id
  LEFT JOIN sequence AS s
    ON o.order_id = s.order_id
)

SELECT *
FROM final
