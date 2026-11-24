MODEL (
  description """
    One row per web tracker event, typed from the raw payload and built in hourly microbatches.
    An event without a client timestamp takes the time the collector received it.
  """,
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor received_at,
  cursor_type timestamp,
  cursor_grain hour,
  batch_size 6h,
  lookback 2h,
  replay_on_change full,
  cursor_inputs (
    clickstream__web_events (column received_at, roles [filter, watermark]),
  ),
  on_schema_change append_new_columns,
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null (run_scope delta_and_final), unique]),
    received_at (type TIMESTAMP_NTZ, audits [not_null]),
    event_type (type VARCHAR, audits [not_null]),
    event_at_utc (type TIMESTAMP_NTZ),
    anonymous_id (type VARCHAR),
    customer_id (type VARCHAR),
    user_email (type VARCHAR),
    page_url (type VARCHAR),
    page_path (type VARCHAR),
    referrer_host (type VARCHAR),
    user_agent (type VARCHAR),
    locale (type VARCHAR),
    client_timezone (type VARCHAR),
    is_bot (type BOOLEAN),
    properties (type OBJECT),
    items (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("clickstream__web_events")
),

deduplicated AS (
  SELECT
    e.event_id,
    e.received_at,
    e.event
  FROM src AS e
  QUALIFY ROW_NUMBER() OVER (PARTITION BY e.event_id ORDER BY e._loaded_at DESC) = 1
),

final AS (
  SELECT
    d.event_id,
    d.received_at,
    LOWER(REGEXP_REPLACE(
      d.event:type::VARCHAR,
      '[^A-Za-z]+',
      '_'
    ))::VARCHAR AS event_type,
    COALESCE(
      @sql.epoch_ms_to_utc(d.event:ts_ms::NUMBER(38, 0)),
      d.received_at
    ) AS event_at_utc,
    LOWER(d.event:anonymous_id::VARCHAR)::VARCHAR AS anonymous_id,
    @sql.json_text(d.event:user.id) AS customer_id,
    @sql.normalise_email(d.event:user.email::VARCHAR) AS user_email,
    d.event:page.url::VARCHAR AS page_url,
    (
      '/' || LTRIM(PARSE_URL(d.event:page.url::VARCHAR, 1)['path']::VARCHAR, '/')
    )::VARCHAR AS page_path,
    LOWER(PARSE_URL(d.event:page.referrer::VARCHAR, 1)['host']::VARCHAR)::VARCHAR AS referrer_host,
    d.event:context.ua::VARCHAR AS user_agent,
    REPLACE(
      d.event:context.locale::VARCHAR,
      '_',
      '-'
    )::VARCHAR AS locale,
    d.event:context.tz::VARCHAR AS client_timezone,
    COALESCE(REGEXP_LIKE(
      d.event:context.ua::VARCHAR,
      '.*(bot|crawler|spider|headless).*',
      'i'
    ), FALSE) AS is_bot,
    COALESCE(d.event:properties, OBJECT_CONSTRUCT())::OBJECT AS properties,
    COALESCE(d.event:items, ARRAY_CONSTRUCT())::ARRAY AS items
  FROM deduplicated AS d
)

SELECT *
FROM final
