MODEL (
  materialized table,
  tags [marts],
  description "Order fact table with waffle and payment details.",
  columns (
    order_id (nullable false, audits [not_null]),
  ),
);

SELECT
  o.order_id,
  o.customer_id,
  o.waffle_type_id,
  w.waffle_name,
  w.category AS waffle_category,
  o.quantity,
  @py.line_total_cents("w.price_cents", "o.quantity") AS line_total_cents,
  o.ordered_at,
  o.status AS order_status,
  __udf("udf__is_completed_order")(o.status) AS is_completed_order,
  p.payment_method,
  p.payment_status,
  p.amount_cents AS payment_amount_cents
FROM __ref("stg_orders") o
LEFT JOIN __seed("waffle_types") w ON o.waffle_type_id = w.waffle_type_id
LEFT JOIN __ref("stg_payments") p ON o.order_id = p.order_id
