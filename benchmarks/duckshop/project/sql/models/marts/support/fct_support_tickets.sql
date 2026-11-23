MODEL (
  description """
    One row per support ticket with priority, first response and resolution against SLA, thread
    activity, chat sentiment and the customer's contact history. Merged incrementally as ticket
    change-log rows arrive.
  """,
  materialized incremental,
  incremental_strategy merge,
  unique_key [ticket_id],
  cursor source_loaded_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    stg_tickets source_loaded_at,
  ),
  replay_on_change bounded-120d,
  on_schema_change sync_all_columns,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR),
    channel (type VARCHAR),
    priority (type ticket_priority),
    ticket_status (type VARCHAR),
    contact_reason (type VARCHAR),
    created_at_utc (type TIMESTAMP_NTZ),
    solved_at (type TIMESTAMP_TZ),
    minutes_to_first_response (type INT),
    business_minutes_to_response (type INT),
    is_first_response_breached (type BOOLEAN),
    is_resolution_breached (type BOOLEAN),
    comment_count (type INT),
    handoff_count (type INT),
    sentiment_label (type VARCHAR),
    ticket_number_for_customer (type INT),
    is_repeat_contact_7d (type BOOLEAN),
    csat_score (type INT),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false, audits [not_null]),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

first_responses AS (
  SELECT *
  FROM __ref("int_ticket_first_response")
),

business_hours AS (
  SELECT *
  FROM __ref("int_ticket_business_hours")
),

breaches AS (
  SELECT *
  FROM __ref("int_ticket_sla_breaches")
),

threads AS (
  SELECT *
  FROM __ref("int_ticket_threads")
),

chats AS (
  SELECT *
  FROM __ref("int_chat_sentiment")
),

history AS (
  SELECT *
  FROM __ref("int_customer_support_history")
),

changed_tickets AS (
  SELECT
    t.ticket_id,
    t.channel,
    t.ticket_status,
    t.contact_reason,
    t.solved_at,
    t.source_loaded_at
  FROM tickets AS t
  WHERE
    t.source_loaded_at >= __cursor_start()
    AND t.source_loaded_at < __cursor_end()
),

ticket_sentiment AS (
  SELECT
    c.ticket_id,
    MIN_BY(c.sentiment_label, c.sentiment_score) AS sentiment_label
  FROM chats AS c
  WHERE c.ticket_id IS NOT NULL
  GROUP BY
    c.ticket_id
),

final AS (
  SELECT
    t.ticket_id,
    h.customer_id,
    t.channel,
    b.priority,
    t.ticket_status,
    t.contact_reason,
    f.created_at_utc,
    t.solved_at,
    f.minutes_to_first_response,
    bh.business_minutes_to_response,
    COALESCE(b.is_first_response_breached, FALSE)::BOOLEAN AS is_first_response_breached,
    COALESCE(b.is_resolution_breached, FALSE)::BOOLEAN AS is_resolution_breached,
    COALESCE(th.comment_count, 0)::INT AS comment_count,
    COALESCE(th.handoff_count, 0)::INT AS handoff_count,
    s.sentiment_label::VARCHAR AS sentiment_label,
    h.ticket_number_for_customer,
    COALESCE(h.is_repeat_contact_7d, FALSE)::BOOLEAN AS is_repeat_contact_7d,
    h.csat_score,
    t.source_loaded_at
  FROM changed_tickets AS t
  LEFT JOIN first_responses AS f
    ON t.ticket_id = f.ticket_id
  LEFT JOIN business_hours AS bh
    ON t.ticket_id = bh.ticket_id
  LEFT JOIN breaches AS b
    ON t.ticket_id = b.ticket_id
  LEFT JOIN threads AS th
    ON t.ticket_id = th.ticket_id
  LEFT JOIN ticket_sentiment AS s
    ON t.ticket_id = s.ticket_id
  LEFT JOIN history AS h
    ON t.ticket_id = h.ticket_id
)

SELECT *
FROM final
