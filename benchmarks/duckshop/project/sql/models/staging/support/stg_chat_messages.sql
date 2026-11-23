MODEL (
  description "One row per chat message, flattened from the transcript with masked contact details",
  columns (
    chat_message_key (type VARCHAR, nullable false, audits [not_null, unique]),
    chat_id (type VARCHAR, audits [not_null]),
    message_index (type INT),
    sender_role (type VARCHAR),
    sent_at (type TIMESTAMP_TZ),
    message_text_masked (type VARCHAR),
    message_length (type INT),
  ),
);

WITH chats AS (
  SELECT *
  FROM __ref("stg_chat_transcripts")
),

messages AS (
  SELECT
    c.chat_id,
    m.index::INT AS message_index,
    LOWER(m.value:role::VARCHAR) AS sender_role,
    TRY_TO_TIMESTAMP_TZ(m.value:ts::VARCHAR) AS sent_at,
    m.value:text::VARCHAR AS message_text
  FROM chats AS c, LATERAL FLATTEN(input => c.messages) AS m
),

final AS (
  SELECT
    (
      m.chat_id || '#' || m.message_index::VARCHAR
    )::VARCHAR AS chat_message_key,
    m.chat_id,
    m.message_index,
    m.sender_role::VARCHAR AS sender_role,
    m.sent_at,
    REGEXP_REPLACE(
      REGEXP_REPLACE(m.message_text, '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+', '<email>'),
      '\\+?[0-9][0-9 ()-]{6,}[0-9]',
      '<phone>'
    )::VARCHAR AS message_text_masked,
    LENGTH(m.message_text)::INT AS message_length
  FROM messages AS m
)

SELECT *
FROM final
