MODEL (
  description """
    One row per ticket with the first public agent response and the first customer reply after it.
    Ticket creation is a TIMESTAMP_LTZ in the source and is converted explicitly to UTC here; the
    conversion depends on the session time zone, an expected divergence across engines.
  """,
  tags [known_divergence],
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    created_at_utc (type TIMESTAMP_NTZ, audits [not_null]),
    first_response_at_utc (type TIMESTAMP_NTZ),
    first_responder_id (type VARCHAR),
    minutes_to_first_response (type INT),
    first_customer_reply_at_utc (type TIMESTAMP_NTZ),
    agent_responses_before_customer_reply (type INT),
    has_auto_reply (type BOOLEAN),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

comments AS (
  SELECT *
  FROM __ref("stg_ticket_comments")
),

tickets_utc AS (
  SELECT
    t.ticket_id,
    CONVERT_TIMEZONE('UTC', t.created_at::TIMESTAMP_LTZ)::TIMESTAMP_NTZ AS created_at_utc
  FROM tickets AS t
),

comments_utc AS (
  SELECT
    c.ticket_id,
    c.author_type,
    c.author_id,
    c.is_public,
    CONVERT_TIMEZONE('UTC', c.created_at)::TIMESTAMP_NTZ AS created_at_utc
  FROM comments AS c
),

response_windows AS (
  SELECT
    c.ticket_id,
    c.author_type,
    c.author_id,
    c.created_at_utc,
    MIN(
      IFF(c.author_type = 'agent' AND c.is_public, c.created_at_utc, NULL)
    ) OVER (PARTITION BY c.ticket_id) AS first_response_at_utc
  FROM comments_utc AS c
),

first_responses AS (
  SELECT
    r.ticket_id,
    MIN(r.first_response_at_utc) AS first_response_at_utc,
    MIN_BY(
      IFF(r.author_type = 'agent', r.author_id, NULL),
      IFF(r.author_type = 'agent', r.created_at_utc, NULL)
    ) AS first_responder_id,
    MIN(
      IFF(
        r.author_type = 'customer' AND r.created_at_utc > r.first_response_at_utc,
        r.created_at_utc,
        NULL
      )
    ) AS first_customer_reply_at_utc,
    COUNT_IF(r.author_type = 'system') > 0 AS has_auto_reply
  FROM response_windows AS r
  GROUP BY
    r.ticket_id
),

agent_burst AS (
  SELECT
    r.ticket_id,
    COUNT(*) AS agent_responses_before_customer_reply
  FROM response_windows AS r
  INNER JOIN first_responses AS f
    ON r.ticket_id = f.ticket_id
  WHERE
    r.author_type = 'agent'
    AND r.created_at_utc < COALESCE(f.first_customer_reply_at_utc, '9999-12-31'::TIMESTAMP_NTZ)
  GROUP BY
    r.ticket_id
),

final AS (
  SELECT
    t.ticket_id,
    t.created_at_utc,
    f.first_response_at_utc,
    f.first_responder_id::VARCHAR AS first_responder_id,
    DATEDIFF('minute', t.created_at_utc, f.first_response_at_utc)::INT AS minutes_to_first_response,
    f.first_customer_reply_at_utc,
    COALESCE(
      a.agent_responses_before_customer_reply,
      0
    )::INT AS agent_responses_before_customer_reply,
    COALESCE(f.has_auto_reply, FALSE)::BOOLEAN AS has_auto_reply
  FROM tickets_utc AS t
  LEFT JOIN first_responses AS f
    ON t.ticket_id = f.ticket_id
  LEFT JOIN agent_burst AS a
    ON t.ticket_id = a.ticket_id
)

SELECT *
FROM final
