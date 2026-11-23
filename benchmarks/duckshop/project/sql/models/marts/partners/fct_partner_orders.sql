MODEL (
  description """
    One row per partner order in one shape across payload versions, with USD totals and the matched
    customer
  """,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    partner_order_key (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    payload_version (type INT, audits [accepted_values (values [1, 2])]),
    external_order_id (type VARCHAR),
    currency_code (type VARCHAR),
    placed_at_utc (type TIMESTAMP_NTZ),
    placed_date (type DATE),
    order_total_usd (type NUMBER(18, 2), audits [non_negative]),
    line_count (type INT),
    units (type INT),
    lines_match_total (type BOOLEAN),
    customer_id (type VARCHAR),
    is_known_customer (type BOOLEAN),
  ),
);

WITH partner_orders AS (
  SELECT *
  FROM __ref("int_partner_orders_unified")
),

final AS (
  SELECT
    o.partner_order_key,
    o.partner_code,
    o.payload_version,
    o.external_order_id,
    o.currency_code,
    o.placed_at_utc,
    o.placed_date,
    o.order_total_usd,
    o.line_count,
    o.units,
    o.lines_match_total,
    o.customer_id,
    o.is_known_customer
  FROM partner_orders AS o
)

SELECT *
FROM final
