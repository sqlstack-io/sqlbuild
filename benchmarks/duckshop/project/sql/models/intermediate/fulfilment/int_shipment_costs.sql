MODEL (
  description """
    One row per shipment package with its chargeable weight (the greater of actual and volumetric)
    and the carrier rate from the matching rate card weight band.
  """,
  columns (
    package_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    carrier_code (type VARCHAR),
    service_level (type VARCHAR),
    chargeable_weight_kg (type NUMBER(10, 3)),
    weight_basis (type VARCHAR),
    band_max_kg (type NUMBER(10, 3)),
    rate_minor (type INT),
    rate_currency_code (type VARCHAR),
    has_rate (type BOOLEAN),
  ),
);

WITH packages AS (
  SELECT *
  FROM __ref("stg_shipment_packages")
),

shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_enriched")
),

rate_cards AS (
  SELECT *
  FROM __ref("stg_carrier_rate_cards")
),

package_lanes AS (
  SELECT
    p.package_id,
    p.shipment_id,
    s.carrier_code,
    s.service_level,
    w.country_code AS origin_country,
    o.ship_to_country_code AS destination_country,
    s.shipped_at::DATE AS shipped_date,
    GREATEST(p.weight_grams / 1000, COALESCE(p.volumetric_weight_kg, 0)) AS chargeable_weight_kg,
    IFF(
      COALESCE(p.volumetric_weight_kg, 0) > p.weight_grams / 1000,
      'volumetric',
      'actual'
    ) AS weight_basis
  FROM packages AS p
  INNER JOIN shipments AS s
    ON p.shipment_id = s.shipment_id
  INNER JOIN warehouses AS w
    ON s.warehouse_code = w.warehouse_code
  LEFT JOIN orders AS o
    ON s.order_id = o.order_id
),

bands AS (
  SELECT
    r.carrier_code,
    r.service_level,
    r.origin_country,
    r.destination_country,
    r.currency_code,
    r.valid_from,
    r.valid_to,
    b.value:max_kg::NUMBER(10, 3) AS band_max_kg,
    b.value:amount_minor::INT AS rate_minor
  FROM rate_cards AS r, LATERAL FLATTEN(input => r.bands) AS b
),

matched AS (
  SELECT
    l.package_id,
    MIN(b.band_max_kg) AS band_max_kg,
    MIN_BY(b.rate_minor, b.band_max_kg) AS rate_minor,
    MIN_BY(b.currency_code, b.band_max_kg) AS rate_currency_code
  FROM package_lanes AS l
  INNER JOIN bands AS b
    ON l.carrier_code = b.carrier_code
    AND l.service_level = b.service_level
    AND l.origin_country = b.origin_country
    AND l.destination_country = b.destination_country
    AND l.shipped_date BETWEEN b.valid_from AND b.valid_to
    AND l.chargeable_weight_kg <= b.band_max_kg
  GROUP BY
    l.package_id
),

final AS (
  SELECT
    l.package_id,
    l.shipment_id,
    l.carrier_code,
    l.service_level,
    ROUND(l.chargeable_weight_kg, 3)::NUMBER(10, 3) AS chargeable_weight_kg,
    l.weight_basis::VARCHAR AS weight_basis,
    m.band_max_kg::NUMBER(10, 3) AS band_max_kg,
    m.rate_minor::INT AS rate_minor,
    m.rate_currency_code::VARCHAR AS rate_currency_code,
    (
      m.rate_minor IS NOT NULL
    ) AS has_rate
  FROM package_lanes AS l
  LEFT JOIN matched AS m
    ON l.package_id = m.package_id
)

SELECT *
FROM final
