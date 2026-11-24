MODEL (
  description """
    One row per tracking event of a shipment: carrier scans flattened from tracking, plus recorded
    delivery exceptions
  """,
  columns (
    tracking_event_key (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR, audits [not_null]),
    event_source (type VARCHAR, audits [accepted_values (values ["scan", "exception"])]),
    event_code (type VARCHAR),
    event_at (type TIMESTAMP_TZ),
    location_text (type VARCHAR),
    location_country_code (type VARCHAR),
    event_sequence (type INT),
  ),
);

WITH shipments AS (
  SELECT *
  FROM __ref("stg_shipments")
),

exceptions AS (
  SELECT *
  FROM __ref("stg_delivery_exceptions")
),

scans AS (
  SELECT
    s.shipment_id,
    e.index::INT AS event_sequence,
    e.value:code::VARCHAR AS raw_code,
    COALESCE(TRY_TO_TIMESTAMP_TZ(e.value:ts::VARCHAR), TRY_TO_TIMESTAMP_TZ(
      e.value:ts::VARCHAR,
      'DD/MM/YYYY HH24:MI TZHTZM'
    )) AS event_at,
    e.value:location::VARCHAR AS raw_location
  FROM shipments AS s, LATERAL FLATTEN(input => s.tracking_events) AS e
),

scan_events AS (
  SELECT
    (
      s.shipment_id || ':' || s.event_sequence::VARCHAR
    ) AS tracking_event_key,
    s.shipment_id,
    'scan' AS event_source,
    UPPER(REGEXP_REPLACE(s.raw_code, '[^A-Za-z_]', '')) AS event_code,
    s.event_at,
    INITCAP(TRIM(SPLIT_PART(s.raw_location, ',', 1))) AS location_text,
    UPPER(
      REGEXP_SUBSTR(s.raw_location, ',\\s*([A-Za-z]{2})\\s*$', 1, 1, 'e', 1)
    ) AS location_country_code,
    s.event_sequence
  FROM scans AS s
),

exception_events AS (
  SELECT
    (
      x.shipment_id || ':x:' || x.exception_id
    ) AS tracking_event_key,
    x.shipment_id,
    'exception' AS event_source,
    (
      'EXCEPTION_' || x.exception_code
    ) AS event_code,
    x.occurred_at AS event_at,
    NULL AS location_text,
    NULL AS location_country_code,
    NULL AS event_sequence
  FROM exceptions AS x
),

final AS (
  SELECT
    se.tracking_event_key::VARCHAR AS tracking_event_key,
    se.shipment_id,
    se.event_source::VARCHAR AS event_source,
    se.event_code::VARCHAR AS event_code,
    se.event_at,
    se.location_text::VARCHAR AS location_text,
    se.location_country_code::VARCHAR AS location_country_code,
    se.event_sequence::INT AS event_sequence
  FROM scan_events AS se
  UNION ALL
  SELECT
    ee.tracking_event_key::VARCHAR AS tracking_event_key,
    ee.shipment_id,
    ee.event_source::VARCHAR AS event_source,
    ee.event_code::VARCHAR AS event_code,
    ee.event_at,
    ee.location_text::VARCHAR AS location_text,
    ee.location_country_code::VARCHAR AS location_country_code,
    ee.event_sequence::INT AS event_sequence
  FROM exception_events AS ee
)

SELECT *
FROM final
