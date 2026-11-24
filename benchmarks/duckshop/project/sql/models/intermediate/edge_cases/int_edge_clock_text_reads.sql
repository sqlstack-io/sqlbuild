MODEL (
  description """
    One row per partner clock text read by conversion functions: DATE and TO_TIME of the text's date
    and time parts, TRY_TO_TIMESTAMP of the whole text, and TO_TIMESTAMP_LTZ of zoned text read back
    as the instant at UTC, so the session time zone does not change it. CURRENT_DATE and SYSDATE are
    read only for facts that hold on any day: the date is after 2020, and the date differs from
    itself by zero days
  """,
  tags [edge_case],
  columns (
    text_id (type VARCHAR, nullable false, audits [not_null, unique]),
    clock_date (type DATE),
    clock_time (type TIME),
    clock_timestamp (type TIMESTAMP_NTZ),
    zoned_instant_utc (type TIMESTAMP_NTZ),
    today_is_after_2020 (type BOOLEAN),
    today_minus_today (type NUMBER(38, 0)),
    system_clock_is_after_2020 (type BOOLEAN),
  ),
);

WITH texts AS (
  SELECT
    t.text_id,
    t.zoned_text,
    t.wall_clock_text
  FROM __ref("stg_edge_date_texts") AS t
),

final AS (
  SELECT
    t.text_id,
    DATE(SPLIT_PART(t.wall_clock_text, ' ', 1))::DATE AS clock_date,
    TO_TIME(SPLIT_PART(t.wall_clock_text, ' ', 2))::TIME AS clock_time,
    TRY_TO_TIMESTAMP(t.wall_clock_text)::TIMESTAMP_NTZ AS clock_timestamp,
    CONVERT_TIMEZONE('UTC', TO_TIMESTAMP_LTZ(t.zoned_text))::TIMESTAMP_NTZ AS zoned_instant_utc,
    (CURRENT_DATE >= '2020-01-01'::DATE)::BOOLEAN AS today_is_after_2020,
    DATEDIFF('day', CURRENT_DATE, CURRENT_DATE)::NUMBER(38, 0) AS today_minus_today,
    (SYSDATE() >= '2020-01-01 00:00:00'::TIMESTAMP_NTZ)::BOOLEAN AS system_clock_is_after_2020
  FROM texts AS t
)

SELECT *
FROM final
