MODEL (
  description """
    One row per delivery exception with a responsibility class, its time to resolve and compensation
    in USD
  """,
  columns (
    exception_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    carrier_code (type VARCHAR),
    exception_code (type VARCHAR),
    responsibility (type VARCHAR, audits [
      accepted_values (values ["carrier", "customer", "warehouse", "unknown"]),
    ]),
    occurred_at (type TIMESTAMP_TZ),
    hours_to_resolve (type NUMBER(10, 2)),
    resolution_type (type VARCHAR),
    compensation_usd (type NUMBER(18, 2)),
    exception_rank_in_shipment (type INT),
  ),
);

WITH exceptions AS (
  SELECT *
  FROM __ref("stg_delivery_exceptions")
),

shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_usd")
),

classified AS (
  SELECT
    e.exception_id,
    e.shipment_id,
    e.exception_code,
    e.occurred_at,
    e.resolved_at,
    e.resolution_type,
    e.compensation_minor,
    CASE
      WHEN REGEXP_LIKE(e.exception_code, '^(ADDR|NOT_HOME|REFUSED|ACCESS).*')
      THEN 'customer'
      WHEN REGEXP_LIKE(e.exception_code, '^(LOST|DAMAGED|DELAY|MISROUTE).*')
      THEN 'carrier'
      WHEN REGEXP_LIKE(e.exception_code, '^(LABEL|PACK|WRONG_ITEM).*')
      THEN 'warehouse'
      ELSE 'unknown'
    END AS responsibility
  FROM exceptions AS e
),

final AS (
  SELECT
    c.exception_id,
    c.shipment_id,
    s.carrier_code,
    c.exception_code,
    c.responsibility::VARCHAR AS responsibility,
    c.occurred_at,
    ROUND(
      DATEDIFF('minute', c.occurred_at, c.resolved_at) / 60,
      2
    )::NUMBER(10, 2) AS hours_to_resolve,
    c.resolution_type,
    ROUND(
      c.compensation_minor / POWER(10, @py.minor_unit_exponent("o.currency_code")) * o.rate_to_usd,
      2
    )::NUMBER(18, 2) AS compensation_usd,
    RANK() OVER (
      PARTITION BY c.shipment_id
      ORDER BY c.occurred_at
    )::INT AS exception_rank_in_shipment
  FROM classified AS c
  INNER JOIN shipments AS s
    ON c.shipment_id = s.shipment_id
  LEFT JOIN orders AS o
    ON s.order_id = o.order_id
)

SELECT *
FROM final
