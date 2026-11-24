MODEL (
  description """
    One row per chat with a keyword-based sentiment score for the customer's messages, agent
    response latency, and whether the chat ended with a thank-you.
  """,
  constants (
    _negative_pattern "(angry|terrible|worst|refund|cancel|broken|never again|ridiculous)",
    _positive_pattern "(thanks|thank you|great|perfect|helpful|appreciate|sorted)",
  ),
  columns (
    chat_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR),
    customer_message_count (type INT),
    agent_message_count (type INT),
    sentiment_score (type INT),
    sentiment_label (type VARCHAR, audits [
      accepted_values (values ["negative", "neutral", "positive"]),
    ]),
    median_agent_reply_seconds (type INT),
    ends_with_thanks (type BOOLEAN),
    chat_minutes (type NUMBER(10, 2)),
  ),
);

WITH messages AS (
  SELECT *
  FROM __ref("stg_chat_messages")
),

chats AS (
  SELECT *
  FROM __ref("stg_chat_transcripts")
),

with_previous AS (
  SELECT
    m.chat_id,
    m.message_index,
    m.sender_role,
    m.sent_at,
    LOWER(m.message_text_masked) AS message_text,
    LAG(m.sender_role) OVER (PARTITION BY m.chat_id ORDER BY m.message_index) AS previous_role,
    LAG(m.sent_at) OVER (PARTITION BY m.chat_id ORDER BY m.message_index) AS previous_sent_at
  FROM messages AS m
),

scored AS (
  SELECT
    w.chat_id,
    w.message_index,
    w.sender_role,
    w.message_text,
    IFF(
      w.sender_role = 'customer',
      REGEXP_COUNT(w.message_text, @const("_positive_pattern"))
        - REGEXP_COUNT(w.message_text, @const("_negative_pattern")),
      0
    ) AS message_sentiment,
    IFF(
      w.sender_role = 'agent' AND w.previous_role = 'customer',
      DATEDIFF('second', w.previous_sent_at, w.sent_at),
      NULL
    ) AS agent_reply_seconds
  FROM with_previous AS w
),

per_chat AS (
  SELECT
    s.chat_id,
    COUNT_IF(s.sender_role = 'customer') AS customer_message_count,
    COUNT_IF(s.sender_role = 'agent') AS agent_message_count,
    SUM(s.message_sentiment) AS sentiment_score,
    MEDIAN(s.agent_reply_seconds) AS median_agent_reply_seconds,
    REGEXP_LIKE(
      MAX_BY(s.message_text, s.message_index),
      '.*(thanks|thank you|cheers).*'
    ) AS ends_with_thanks
  FROM scored AS s
  GROUP BY
    s.chat_id
),

final AS (
  SELECT
    c.chat_id,
    c.ticket_id,
    COALESCE(p.customer_message_count, 0)::INT AS customer_message_count,
    COALESCE(p.agent_message_count, 0)::INT AS agent_message_count,
    COALESCE(p.sentiment_score, 0)::INT AS sentiment_score,
    CASE
      WHEN COALESCE(p.sentiment_score, 0) < 0
      THEN 'negative'
      WHEN COALESCE(p.sentiment_score, 0) > 0
      THEN 'positive'
      ELSE 'neutral'
    END::VARCHAR AS sentiment_label,
    p.median_agent_reply_seconds::INT AS median_agent_reply_seconds,
    COALESCE(p.ends_with_thanks, FALSE)::BOOLEAN AS ends_with_thanks,
    ROUND(DATEDIFF('second', c.started_at, c.ended_at) / 60, 2)::NUMBER(10, 2) AS chat_minutes
  FROM chats AS c
  LEFT JOIN per_chat AS p
    ON c.chat_id = p.chat_id
)

SELECT *
FROM final
