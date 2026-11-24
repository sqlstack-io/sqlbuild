MODEL (
  description """
    One row per carrier from the carriers seed with the services and lanes on its current rate cards
    and its delivery record.
  """,
  contract enforced,
  columns (
    carrier_code (type VARCHAR, nullable false, audits [not_null, unique]),
    carrier_name (type VARCHAR),
    is_express (type BOOLEAN),
    tracking_host (type VARCHAR),
    service_levels (type ARRAY),
    destination_country_count (type INT),
    active_rate_card_count (type INT),
    shipment_count (type INT),
    on_time_dispatch_rate (type NUMBER(9, 4)),
    median_hours_to_deliver (type NUMBER(10, 2)),
  ),
);

WITH carriers AS (
  SELECT *
  FROM __seed("carriers")
),

rate_cards AS (
  SELECT *
  FROM __ref("stg_carrier_rate_cards")
),

sla AS (
  SELECT *
  FROM __ref("int_fulfilment_sla")
),

current_cards AS (
  SELECT
    r.carrier_code,
    ARRAY_AGG(DISTINCT r.service_level) WITHIN GROUP (ORDER BY r.service_level) AS service_levels,
    COUNT(DISTINCT r.destination_country) AS destination_country_count,
    COUNT(*) AS active_rate_card_count
  FROM rate_cards AS r
  WHERE
    r.valid_from <= '@@valuation_date'::DATE
    AND (
      r.valid_to IS NULL
      OR r.valid_to > '@@valuation_date'::DATE
    )
  GROUP BY
    r.carrier_code
),

delivery_record AS (
  SELECT
    s.carrier_code,
    COUNT(*) AS shipment_count,
    AVG(IFF(s.is_dispatch_on_time, 1, 0)) AS on_time_dispatch_rate,
    MEDIAN(s.hours_to_deliver) AS median_hours_to_deliver
  FROM sla AS s
  GROUP BY
    s.carrier_code
),

final AS (
  SELECT
    c.carrier_code::VARCHAR AS carrier_code,
    c.carrier_name::VARCHAR AS carrier_name,
    c.is_express::BOOLEAN AS is_express,
    REGEXP_SUBSTR(
      c.tracking_url_template,
      '^https?://([^/]+)',
      1,
      1,
      'e',
      1
    )::VARCHAR AS tracking_host,
    COALESCE(r.service_levels, ARRAY_CONSTRUCT())::ARRAY AS service_levels,
    COALESCE(r.destination_country_count, 0)::INT AS destination_country_count,
    COALESCE(r.active_rate_card_count, 0)::INT AS active_rate_card_count,
    COALESCE(d.shipment_count, 0)::INT AS shipment_count,
    ROUND(d.on_time_dispatch_rate, 4)::NUMBER(9, 4) AS on_time_dispatch_rate,
    ROUND(d.median_hours_to_deliver, 2)::NUMBER(10, 2) AS median_hours_to_deliver
  FROM carriers AS c
  LEFT JOIN current_cards AS r
    ON c.carrier_code = r.carrier_code
  LEFT JOIN delivery_record AS d
    ON c.carrier_code = d.carrier_code
)

SELECT *
FROM final
