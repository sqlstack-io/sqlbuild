MODEL (
  description """
    One row per ticket linked to a customer account by email, with the customer's ticket sequence,
    days since their previous ticket, and their satisfaction trend.
  """,
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR),
    ticket_number_for_customer (type INT),
    days_since_previous_ticket (type INT),
    csat_score (type INT),
    rolling_csat_avg (type NUMBER(6, 3)),
    is_repeat_contact_7d (type BOOLEAN),
    contact_reason (type VARCHAR),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

identities AS (
  SELECT *
  FROM __ref("int_customer_identity")
),

csat AS (
  SELECT *
  FROM __ref("stg_csat_responses")
),

latest_csat AS (
  SELECT
    c.ticket_id,
    c.score
  FROM csat AS c
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY c.ticket_id
      ORDER BY c.responded_at DESC, c.response_id DESC
    ) = 1
),

email_owners AS (
  SELECT
    i.email_normalised,
    MIN(i.customer_id) AS customer_id
  FROM identities AS i
  WHERE i.email_normalised IS NOT NULL
  GROUP BY
    i.email_normalised
),

linked AS (
  SELECT
    t.ticket_id,
    i.customer_id,
    CONVERT_TIMEZONE('UTC', t.created_at)::TIMESTAMP_NTZ AS created_at_utc,
    t.contact_reason,
    lc.score AS csat_score
  FROM tickets AS t
  LEFT JOIN email_owners AS i
    ON t.customer_email = i.email_normalised
  LEFT JOIN latest_csat AS lc
    ON t.ticket_id = lc.ticket_id
),

sequenced AS (
  SELECT
    l.ticket_id,
    l.customer_id,
    l.created_at_utc,
    l.contact_reason,
    l.csat_score,
    ROW_NUMBER() OVER (
      PARTITION BY l.customer_id
      ORDER BY l.created_at_utc, l.ticket_id
    ) AS ticket_number_for_customer,
    LAG(l.created_at_utc) OVER (
      PARTITION BY l.customer_id
      ORDER BY l.created_at_utc, l.ticket_id
    ) AS previous_created_at_utc,
    AVG(l.csat_score) OVER (
      PARTITION BY l.customer_id
      ORDER BY l.created_at_utc, l.ticket_id
      ROWS BETWEEN 2 PRECEDING AND CURRENT ROW
    ) AS rolling_csat_avg
  FROM linked AS l
),

final AS (
  SELECT
    s.ticket_id,
    s.customer_id,
    IFF(
      s.customer_id IS NULL,
      NULL,
      s.ticket_number_for_customer
    )::INT AS ticket_number_for_customer,
    DATEDIFF('day', s.previous_created_at_utc, s.created_at_utc)::INT AS days_since_previous_ticket,
    s.csat_score,
    ROUND(s.rolling_csat_avg, 3)::NUMBER(6, 3) AS rolling_csat_avg,
    COALESCE(
      DATEDIFF('hour', s.previous_created_at_utc, s.created_at_utc) <= 168,
      FALSE
    ) AS is_repeat_contact_7d,
    s.contact_reason
  FROM sequenced AS s
)

SELECT *
FROM final
