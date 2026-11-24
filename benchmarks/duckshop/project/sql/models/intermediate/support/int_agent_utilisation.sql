MODEL (
  description """
    One row per agent and shift with scheduled productive minutes and the minutes spent on calls and
    first responses during the shift, with shift times converted from the agent's zone to UTC.
  """,
  columns (
    shift_id (type VARCHAR, nullable false, audits [not_null, unique]),
    agent_id (type VARCHAR, audits [not_null]),
    team_code (type VARCHAR),
    shift_start_utc (type TIMESTAMP_NTZ),
    shift_end_utc (type TIMESTAMP_NTZ),
    productive_minutes (type INT),
    call_minutes (type NUMBER(10, 2)),
    calls_handled (type INT),
    tickets_first_responded (type INT),
    utilisation (type NUMBER(9, 4)),
    is_overnight (type BOOLEAN),
  ),
);

WITH shifts AS (
  SELECT *
  FROM __ref("stg_agent_shifts")
),

agents AS (
  SELECT *
  FROM __ref("stg_agents")
),

calls AS (
  SELECT *
  FROM __ref("stg_call_records")
),

first_responses AS (
  SELECT *
  FROM __ref("int_ticket_first_response")
),

shifts_utc AS (
  SELECT
    s.shift_id,
    s.agent_id,
    CONVERT_TIMEZONE(s.iana_timezone, 'UTC', s.shift_start_local)::TIMESTAMP_NTZ AS shift_start_utc,
    CONVERT_TIMEZONE(s.iana_timezone, 'UTC', s.shift_end_local)::TIMESTAMP_NTZ AS shift_end_utc,
    s.scheduled_minutes - s.break_minutes AS productive_minutes,
    (
      s.shift_end_local::DATE > s.shift_start_local::DATE
    ) AS is_overnight
  FROM shifts AS s
),

calls_in_shift AS (
  SELECT
    su.shift_id,
    COUNT(c.call_id) AS calls_handled,
    SUM(c.duration_seconds) / 60 AS call_minutes
  FROM shifts_utc AS su
  INNER JOIN calls AS c
    ON su.agent_id = c.agent_id
  WHERE
    CONVERT_TIMEZONE(
      'UTC',
      c.started_at
    )::TIMESTAMP_NTZ BETWEEN su.shift_start_utc AND su.shift_end_utc
  GROUP BY
    su.shift_id
),

responses_in_shift AS (
  SELECT
    su.shift_id,
    COUNT(f.ticket_id) AS tickets_first_responded
  FROM shifts_utc AS su
  INNER JOIN first_responses AS f
    ON su.agent_id = f.first_responder_id
    AND f.first_response_at_utc >= su.shift_start_utc
    AND f.first_response_at_utc < su.shift_end_utc
  GROUP BY
    su.shift_id
),

final AS (
  SELECT
    su.shift_id,
    su.agent_id,
    a.team_code,
    su.shift_start_utc,
    su.shift_end_utc,
    su.productive_minutes::INT AS productive_minutes,
    ROUND(COALESCE(c.call_minutes, 0), 2)::NUMBER(10, 2) AS call_minutes,
    COALESCE(c.calls_handled, 0)::INT AS calls_handled,
    COALESCE(r.tickets_first_responded, 0)::INT AS tickets_first_responded,
    ROUND(
      (
        COALESCE(c.call_minutes, 0) + 6 * COALESCE(r.tickets_first_responded, 0)
      )
      / NULLIF(su.productive_minutes, 0),
      4
    )::NUMBER(9, 4) AS utilisation,
    su.is_overnight
  FROM shifts_utc AS su
  LEFT JOIN agents AS a
    ON su.agent_id = a.agent_id
  LEFT JOIN calls_in_shift AS c
    ON su.shift_id = c.shift_id
  LEFT JOIN responses_in_shift AS r
    ON su.shift_id = r.shift_id
)

SELECT *
FROM final
