MODEL (
  description "One row per live agent shift in the agent's local time with paid break minutes",
  columns (
    shift_id (type VARCHAR, nullable false, audits [not_null, unique]),
    agent_id (type VARCHAR, audits [not_null]),
    shift_start_local (type TIMESTAMP_NTZ),
    shift_end_local (type TIMESTAMP_NTZ),
    iana_timezone (type VARCHAR),
    scheduled_minutes (type INT),
    break_minutes (type INT),
    breaks (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__agent_shifts")
),

@sql.cdc_latest(src, src.key := (shift_id))

breaks AS (
  SELECT
    s.shift_id,
    SUM(COALESCE(b.value:minutes::INT, 0)) AS break_minutes
  FROM cdc_latest AS s, LATERAL FLATTEN(input => s.breaks, outer => TRUE) AS b
  GROUP BY
    s.shift_id
), final AS (
  SELECT
    s.shift_id,
    s.agent_id,
    s.shift_start_local,
    s.shift_end_local,
    s.iana_timezone,
    DATEDIFF('minute', s.shift_start_local, s.shift_end_local)::INT AS scheduled_minutes,
    COALESCE(b.break_minutes, 0)::INT AS break_minutes,
    COALESCE(s.breaks, ARRAY_CONSTRUCT())::ARRAY AS breaks,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
  LEFT JOIN breaks AS b
    ON s.shift_id = b.shift_id
)
SELECT *
FROM final
