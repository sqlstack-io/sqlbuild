MODEL (
  description """
    One row per payment gateway callback, with its free-text time read in every lenient form and its
    epoch milliseconds read as milliseconds, as epoch text and as a plain cast
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    merchant_zone (type VARCHAR),
    local_wall_time (type TIMESTAMP_NTZ),
    event_at (type TIMESTAMP_TZ),
    event_at_ns (type TIMESTAMP_NTZ(9)),
    event_time_text (type VARCHAR),
    business_date (type DATE),
    period_start (type TIMESTAMP_NTZ),
    period_end (type TIMESTAMP_NTZ),
    text_timestamp (type TIMESTAMP_NTZ),
    text_timestamp_tz (type TIMESTAMP_TZ),
    text_date (type DATE),
    text_time (type TIME),
    text_date_cast (type DATE),
    text_timestamp_day_month (type TIMESTAMP_NTZ),
    sent_at_from_ms (type TIMESTAMP_NTZ),
    sent_at_from_text (type TIMESTAMP_NTZ),
    sent_at_cast (type TIMESTAMP_NTZ),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__gateway_callbacks")
),

final AS (
  SELECT
    s.callback_id,
    s.merchant_zone,
    s.local_wall_time,
    s.event_at,
    s.event_at_ns,
    s.event_time_text,
    s.business_date,
    s.period_start,
    s.period_end,
    TRY_TO_TIMESTAMP_NTZ(s.event_time_text) AS text_timestamp,
    TRY_TO_TIMESTAMP_TZ(s.event_time_text) AS text_timestamp_tz,
    TRY_TO_DATE(s.event_time_text) AS text_date,
    TRY_TO_TIME(s.event_time_text)::TIME AS text_time,
    TRY_CAST(s.event_time_text AS DATE) AS text_date_cast,
    TRY_TO_TIMESTAMP_NTZ(s.event_time_text, 'DD-MON-YY HH24:MI') AS text_timestamp_day_month,
    TO_TIMESTAMP_NTZ(s.body:ts_ms::NUMBER(38, 0), 3) AS sent_at_from_ms,
    TRY_TO_TIMESTAMP_NTZ(s.body:ts_ms::VARCHAR) AS sent_at_from_text,
    s.body:ts_ms::TIMESTAMP_NTZ AS sent_at_cast
  FROM src AS s
)

SELECT *
FROM final
