MODEL (
  description "One row per live ticket comment",
  columns (
    comment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR, audits [not_null]),
    author_type (type VARCHAR, audits [accepted_values (values ["agent", "customer", "system"])]),
    author_id (type VARCHAR),
    is_public (type BOOLEAN),
    created_at (type TIMESTAMP_TZ, audits [not_null]),
    body_word_count (type INT),
    contains_order_reference (type BOOLEAN),
    attachment_count (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__ticket_comments")
),

@sql.cdc_latest(src, src.key := (comment_id))

final AS (
  SELECT
    c.comment_id,
    c.ticket_id,
    LOWER(c.author_type)::VARCHAR AS author_type,
    c.author_id,
    COALESCE(c.is_public, TRUE) AS is_public,
    c.created_at,
    REGEXP_COUNT(TRIM(c.body), '\\S+')::INT AS body_word_count,
    RLIKE(c.body, '.*DS-O-[0-9]+.*', 's')::BOOLEAN AS contains_order_reference,
    ARRAY_SIZE(COALESCE(c.attachments, ARRAY_CONSTRUCT()))::INT AS attachment_count,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
