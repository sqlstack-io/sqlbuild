MODEL (
  description """
    One row per ticket matched to its SLA policy by priority and channel, with first-response and
    resolution targets, the time used, and whether each target was breached. Open tickets also carry
    their age at query time from CURRENT_TIMESTAMP(), which is session-dependent and an expected
    divergence between runs and engines.
  """,
  tags [known_divergence],
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    priority (type ticket_priority),
    channel (type VARCHAR),
    policy_id (type VARCHAR),
    first_response_target_minutes (type INT),
    first_response_minutes_used (type INT),
    is_first_response_breached (type BOOLEAN),
    resolution_target_minutes (type INT),
    resolution_minutes_used (type INT),
    is_resolution_breached (type BOOLEAN),
    breach_count (type INT),
    open_minutes_now (type INT),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

policies AS (
  SELECT *
  FROM __ref("stg_sla_policies")
),

first_responses AS (
  SELECT *
  FROM __ref("int_ticket_first_response")
),

business_hours AS (
  SELECT *
  FROM __ref("int_ticket_business_hours")
),

matched AS (
  SELECT
    t.ticket_id,
    t.priority,
    t.channel,
    t.solved_at,
    p.policy_id,
    p.first_response_minutes,
    p.resolution_minutes,
    p.business_hours_only
  FROM tickets AS t
  LEFT JOIN policies AS p
    ON t.priority = p.priority
    AND t.channel = p.channel
),

measured AS (
  SELECT
    m.ticket_id,
    m.priority,
    m.channel,
    m.policy_id,
    m.first_response_minutes,
    m.resolution_minutes,
    IFF(
      m.business_hours_only,
      bh.business_minutes_to_response,
      fr.minutes_to_first_response
    ) AS first_response_minutes_used,
    DATEDIFF(
      'minute',
      fr.created_at_utc,
      CONVERT_TIMEZONE('UTC', m.solved_at)::TIMESTAMP_NTZ
    ) AS resolution_minutes_used,
    IFF(m.solved_at IS NULL, DATEDIFF(
      'minute',
      fr.created_at_utc,
      CONVERT_TIMEZONE('UTC', CURRENT_TIMESTAMP())::TIMESTAMP_NTZ
    ), NULL) AS open_minutes_now
  FROM matched AS m
  INNER JOIN first_responses AS fr
    ON m.ticket_id = fr.ticket_id
  LEFT JOIN business_hours AS bh
    ON m.ticket_id = bh.ticket_id
),

final AS (
  SELECT
    x.ticket_id,
    x.priority,
    x.channel,
    x.policy_id,
    x.first_response_minutes::INT AS first_response_target_minutes,
    x.first_response_minutes_used::INT AS first_response_minutes_used,
    COALESCE(
      x.first_response_minutes_used > x.first_response_minutes,
      FALSE
    ) AS is_first_response_breached,
    x.resolution_minutes::INT AS resolution_target_minutes,
    x.resolution_minutes_used::INT AS resolution_minutes_used,
    COALESCE(x.resolution_minutes_used > x.resolution_minutes, FALSE) AS is_resolution_breached,
    (
      IFF(COALESCE(x.first_response_minutes_used > x.first_response_minutes, FALSE), 1, 0)
        + IFF(COALESCE(x.resolution_minutes_used > x.resolution_minutes, FALSE), 1, 0)
    )::INT AS breach_count,
    x.open_minutes_now::INT AS open_minutes_now
  FROM measured AS x
)

SELECT *
FROM final
