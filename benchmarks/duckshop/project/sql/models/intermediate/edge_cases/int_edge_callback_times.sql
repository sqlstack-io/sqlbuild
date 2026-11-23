MODEL (
  description """
    One row per payment gateway callback with merchant wall-clock times converted across
    daylight-saving changes, zoned times read at their own offsets, and nanosecond times lowered to
    coarser precisions and epoch parts
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    wall_time_utc (type TIMESTAMP_NTZ),
    wall_time_as_zoned (type TIMESTAMP_TZ),
    event_at_utc (type TIMESTAMP_NTZ),
    event_at_merchant (type TIMESTAMP_TZ),
    event_local_date (type DATE),
    event_ns_date (type DATE),
    event_ns_ms (type TIMESTAMP_NTZ(3)),
    event_ns_us (type TIMESTAMP_NTZ(6)),
    event_ns_text (type VARCHAR),
    epoch_second (type NUMBER(38, 0)),
    epoch_millisecond (type NUMBER(38, 0)),
    epoch_microsecond (type NUMBER(38, 0)),
    epoch_nanosecond (type NUMBER(38, 0)),
  ),
);

WITH callbacks AS (
  SELECT
    s.callback_id,
    s.merchant_zone,
    s.local_wall_time,
    s.event_at,
    s.event_at_ns
  FROM __ref("stg_edge_gateway_callbacks") AS s
),

final AS (
  SELECT
    c.callback_id,
    CONVERT_TIMEZONE(c.merchant_zone, 'UTC', c.local_wall_time)::TIMESTAMP_NTZ AS wall_time_utc,
    c.local_wall_time::TIMESTAMP_TZ AS wall_time_as_zoned,
    CONVERT_TIMEZONE('UTC', c.event_at)::TIMESTAMP_NTZ AS event_at_utc,
    CONVERT_TIMEZONE(c.merchant_zone, c.event_at)::TIMESTAMP_TZ AS event_at_merchant,
    c.event_at::DATE AS event_local_date,
    c.event_at_ns::DATE AS event_ns_date,
    c.event_at_ns::TIMESTAMP_NTZ(3) AS event_ns_ms,
    c.event_at_ns::TIMESTAMP_NTZ(6) AS event_ns_us,
    TO_VARCHAR(c.event_at_ns, 'YYYY-MM-DD HH24:MI:SS.FF3')::VARCHAR AS event_ns_text,
    DATE_PART(EPOCH_SECOND, c.event_at_ns)::NUMBER(38, 0) AS epoch_second,
    DATE_PART(EPOCH_MILLISECOND, c.event_at_ns)::NUMBER(38, 0) AS epoch_millisecond,
    DATE_PART(EPOCH_MICROSECOND, c.event_at_ns)::NUMBER(38, 0) AS epoch_microsecond,
    DATE_PART(EPOCH_NANOSECOND, c.event_at_ns)::NUMBER(38, 0) AS epoch_nanosecond
  FROM callbacks AS c
)

SELECT *
FROM final
