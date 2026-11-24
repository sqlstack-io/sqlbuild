MODEL (
  description """
    One row per shipment with its fulfilment method, carrier, dispatch and delivery timing against
    the business-hours promise, package costs, and delivery exceptions.
  """,
  contract enforced,
  audit_factories [fact_table_quality],
  post_hooks [
    inline_sql("ALTER TABLE @@CTX:destination.qualified SET CHANGE_TRACKING = TRUE"),
  ],
  columns (
    shipment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    fulfilment_method (type fulfilment_method),
    warehouse_code (type VARCHAR),
    carrier_code (type VARCHAR),
    service_level (type VARCHAR),
    shipped_at (type TIMESTAMP_TZ),
    delivered_at (type TIMESTAMP_TZ),
    business_days_to_ship (type INT),
    is_dispatch_on_time (type BOOLEAN),
    hours_to_deliver (type NUMBER(10, 2)),
    package_count (type INT),
    chargeable_weight_kg (type NUMBER(10, 3)),
    rate_minor (type INT),
    exception_count (type INT),
    compensation_usd (type NUMBER(18, 2)),
    tracking_event_count (type INT),
  ),
);

WITH shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

sla AS (
  SELECT *
  FROM __ref("int_fulfilment_sla")
),

packages AS (
  SELECT *
  FROM __ref("int_shipment_costs")
),

exceptions AS (
  SELECT *
  FROM __ref("int_delivery_exceptions_classified")
),

package_totals AS (
  SELECT
    p.shipment_id,
    COUNT(*) AS package_count,
    SUM(p.chargeable_weight_kg) AS chargeable_weight_kg,
    SUM(p.rate_minor) AS rate_minor
  FROM packages AS p
  GROUP BY
    p.shipment_id
),

exception_totals AS (
  SELECT
    e.shipment_id,
    COUNT(*) AS exception_count,
    SUM(e.compensation_usd) AS compensation_usd
  FROM exceptions AS e
  GROUP BY
    e.shipment_id
),

final AS (
  SELECT
    s.shipment_id,
    s.order_id,
    CASE
      WHEN s.warehouse_code IS NULL
      THEN @enum("fulfilment_method").PARTNER
      WHEN s.service_level ILIKE '%collect%'
      THEN @enum("fulfilment_method").CLICK_AND_COLLECT
      ELSE @enum("fulfilment_method").HOME_DELIVERY
    END::VARCHAR AS fulfilment_method,
    s.warehouse_code,
    s.carrier_code,
    s.service_level,
    s.shipped_at,
    s.delivered_at,
    l.business_days_to_ship,
    l.is_dispatch_on_time,
    l.hours_to_deliver,
    COALESCE(p.package_count, 0)::INT AS package_count,
    p.chargeable_weight_kg::NUMBER(10, 3) AS chargeable_weight_kg,
    p.rate_minor::INT AS rate_minor,
    COALESCE(e.exception_count, 0)::INT AS exception_count,
    COALESCE(e.compensation_usd, 0)::NUMBER(18, 2) AS compensation_usd,
    s.tracking_event_count
  FROM shipments AS s
  LEFT JOIN sla AS l
    ON s.shipment_id = l.shipment_id
  LEFT JOIN package_totals AS p
    ON s.shipment_id = p.shipment_id
  LEFT JOIN exception_totals AS e
    ON s.shipment_id = e.shipment_id
)

SELECT *
FROM final
