MODEL (
  description """
    One row per agent and month with shifts, utilisation, calls, first responses and customer
    satisfaction
  """,
  columns (
    agent_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    agent_id (type VARCHAR),
    team_code (type VARCHAR),
    shift_month (type DATE),
    shifts (type INT),
    productive_hours (type NUMBER(10, 2)),
    average_utilisation (type NUMBER(9, 4)),
    calls_handled (type INT),
    tickets_first_responded (type INT),
    average_csat (type NUMBER(4, 2)),
    utilisation_rank_in_team (type INT),
  ),
);

WITH utilisation AS (
  SELECT *
  FROM __ref("int_agent_utilisation")
),

first_responses AS (
  SELECT *
  FROM __ref("int_ticket_first_response")
),

history AS (
  SELECT *
  FROM __ref("int_customer_support_history")
),

agent_csat AS (
  SELECT
    f.first_responder_id AS agent_id,
    DATE_TRUNC('month', f.first_response_at_utc) AS response_month,
    AVG(h.csat_score) AS average_csat
  FROM first_responses AS f
  INNER JOIN history AS h
    ON f.ticket_id = h.ticket_id
  WHERE h.csat_score IS NOT NULL
  GROUP BY
    1,
    2
),

monthly AS (
  SELECT
    u.agent_id,
    u.team_code,
    DATE_TRUNC('month', u.shift_start_utc) AS shift_month,
    COUNT(*) AS shifts,
    SUM(u.productive_minutes) / 60 AS productive_hours,
    AVG(u.utilisation) AS average_utilisation,
    SUM(u.calls_handled) AS calls_handled,
    SUM(u.tickets_first_responded) AS tickets_first_responded
  FROM utilisation AS u
  GROUP BY
    1,
    2,
    3
),

final AS (
  SELECT
    (
      m.agent_id || ':' || TO_VARCHAR(m.shift_month, 'YYYY-MM')
    )::VARCHAR AS agent_month_key,
    m.agent_id,
    m.team_code,
    m.shift_month::DATE AS shift_month,
    m.shifts::INT AS shifts,
    ROUND(m.productive_hours, 2)::NUMBER(10, 2) AS productive_hours,
    ROUND(m.average_utilisation, 4)::NUMBER(9, 4) AS average_utilisation,
    m.calls_handled::INT AS calls_handled,
    m.tickets_first_responded::INT AS tickets_first_responded,
    ROUND(c.average_csat, 2)::NUMBER(4, 2) AS average_csat,
    DENSE_RANK() OVER (
      PARTITION BY m.team_code, m.shift_month
      ORDER BY m.average_utilisation DESC NULLS LAST
    )::INT AS utilisation_rank_in_team
  FROM monthly AS m
  LEFT JOIN agent_csat AS c
    ON m.agent_id = c.agent_id
    AND m.shift_month = c.response_month
)

SELECT *
FROM final
