MODEL (
  description "One row per live chat session",
  columns (
    chat_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR),
    started_at (type TIMESTAMP_TZ),
    ended_at (type TIMESTAMP_TZ),
    message_count (type INT),
    messages (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__chat_transcripts")
),

@sql.cdc_latest(src, src.key := (chat_id))

final AS (
  SELECT
    c.chat_id,
    c.ticket_id,
    c.started_at,
    c.ended_at,
    ARRAY_SIZE(c.messages)::INT AS message_count,
    c.messages::ARRAY AS messages,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
