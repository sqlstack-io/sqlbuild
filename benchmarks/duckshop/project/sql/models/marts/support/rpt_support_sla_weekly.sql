MODEL (
  description """
    One row per week, channel and priority with ticket volume, SLA breach rates and response times
  """,
  columns (
    sla_week_key (type VARCHAR, nullable false, audits [not_null, unique]),
    week_start (type DATE),
    channel (type VARCHAR),
    priority (type ticket_priority),
    tickets (type INT),
    first_response_breach_rate (type NUMBER(9, 4)),
    resolution_breach_rate (type NUMBER(9, 4)),
    p50_minutes_to_first_response (type NUMBER(10, 2)),
    p90_business_minutes_to_response (type NUMBER(10, 2)),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("fct_support_tickets")
),

final AS (
  SELECT
    (
      TO_VARCHAR(DATE_TRUNC('week', t.created_at_utc), 'YYYY-MM-DD')
        || ':'
        || t.channel
        || ':'
        || COALESCE(t.priority::VARCHAR, '?')
    )::VARCHAR AS sla_week_key,
    DATE_TRUNC('week', t.created_at_utc)::DATE AS week_start,
    t.channel,
    t.priority::INT AS priority,
    COUNT(*)::INT AS tickets,
    ROUND(
      AVG(IFF(t.is_first_response_breached, 1, 0)),
      4
    )::NUMBER(9, 4) AS first_response_breach_rate,
    ROUND(AVG(IFF(t.is_resolution_breached, 1, 0)), 4)::NUMBER(9, 4) AS resolution_breach_rate,
    ROUND(MEDIAN(t.minutes_to_first_response), 2)::NUMBER(10, 2) AS p50_minutes_to_first_response,
    ROUND(
      PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY t.business_minutes_to_response),
      2
    )::NUMBER(10, 2) AS p90_business_minutes_to_response
  FROM tickets AS t
  WHERE t.created_at_utc IS NOT NULL
  GROUP BY
    DATE_TRUNC('week', t.created_at_utc),
    t.channel,
    t.priority
)

SELECT *
FROM final
