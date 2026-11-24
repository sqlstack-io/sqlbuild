MODEL (
  description "One row per mobile app event, appended incrementally by received time",
  materialized incremental,
  incremental_strategy append,
  cursor received_at,
  cursor_type timestamp,
  cursor_grain second,
  append_cursor_inclusive false,
  cursor_inputs (
    clickstream__app_events received_at,
  ),
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    received_at (type TIMESTAMP_NTZ, audits [not_null]),
    platform (type VARCHAR, audits [accepted_values (values ["phone", "tablet"])]),
    app_version (type VARCHAR),
    app_major_version (type INT),
    event_name (type VARCHAR),
    event_at_utc (type TIMESTAMP_NTZ),
    device_id (type VARCHAR),
    customer_id (type VARCHAR),
    screen_name (type VARCHAR),
    attributes (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("clickstream__app_events")
),

fresh AS (
  SELECT
    e.event_id,
    e.received_at,
    e.platform,
    e.app_version,
    e.payload
  FROM src AS e
  WHERE
    e.received_at >= __cursor_start()
    AND e.received_at < __cursor_end()
  QUALIFY ROW_NUMBER() OVER (PARTITION BY e.event_id ORDER BY e._loaded_at DESC) = 1
),

final AS (
  SELECT
    f.event_id,
    f.received_at,
    LOWER(f.platform)::VARCHAR AS platform,
    f.app_version,
    TRY_TO_NUMBER(
      SPLIT_PART(REGEXP_SUBSTR(f.app_version, '[0-9]+(\\.[0-9]+)*'), '.', 1)
    )::INT AS app_major_version,
    LOWER(f.payload:name::VARCHAR)::VARCHAR AS event_name,
    COALESCE(
      TRY_TO_TIMESTAMP_NTZ(f.payload:ts::VARCHAR),
      @sql.epoch_ms_to_utc(f.payload:ts_ms::NUMBER(38, 0))
    ) AS event_at_utc,
    f.payload:device.id::VARCHAR AS device_id,
    @sql.json_text(f.payload:user_id) AS customer_id,
    f.payload:screen::VARCHAR AS screen_name,
    COALESCE(f.payload:attributes, OBJECT_CONSTRUCT())::OBJECT AS attributes
  FROM fresh AS f
)

SELECT *
FROM final
