MODEL (
  description """
    One row per ticket with its comment thread summarised: participants, ordered author sequence,
    back-and-forth count, and a per-author breakdown object that keeps authors with no comments.
  """,
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    comment_count (type INT),
    public_comment_count (type INT),
    agent_participants (type ARRAY),
    author_sequence (type ARRAY),
    handoff_count (type INT),
    author_breakdown (type OBJECT),
    total_attachments (type INT),
    last_comment_at (type TIMESTAMP_TZ),
    last_author_type (type VARCHAR),
    mentions_order (type BOOLEAN),
  ),
);

WITH comments AS (
  SELECT *
  FROM __ref("stg_ticket_comments")
),

tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

ordered_comments AS (
  SELECT
    c.ticket_id,
    c.comment_id,
    c.author_type,
    c.author_id,
    c.is_public,
    c.created_at,
    c.attachment_count,
    c.contains_order_reference,
    LAG(c.author_type) OVER (
      PARTITION BY c.ticket_id
      ORDER BY c.created_at, c.comment_id
    ) AS previous_author_type
  FROM comments AS c
),

thread_stats AS (
  SELECT
    o.ticket_id,
    COUNT(*) AS comment_count,
    COUNT_IF(o.is_public) AS public_comment_count,
    ARRAY_AGG(DISTINCT IFF(o.author_type = 'agent', o.author_id, NULL)) WITHIN GROUP (
      ORDER BY IFF(o.author_type = 'agent', o.author_id, NULL)
    ) AS agent_participants,
    ARRAY_AGG(o.author_type) WITHIN GROUP (ORDER BY o.created_at, o.comment_id) AS author_sequence,
    COUNT_IF(
      o.previous_author_type IS NOT NULL AND o.previous_author_type <> o.author_type
    ) AS handoff_count,
    SUM(o.attachment_count) AS total_attachments,
    MAX(o.created_at) AS last_comment_at,
    MAX_BY(o.author_type, o.created_at) AS last_author_type,
    BOOLOR_AGG(o.contains_order_reference) AS mentions_order,
    COUNT_IF(o.author_type = 'agent') AS agent_comments,
    COUNT_IF(o.author_type = 'customer') AS customer_comments,
    COUNT_IF(o.author_type = 'system') AS system_comments
  FROM ordered_comments AS o
  GROUP BY
    o.ticket_id
),

final AS (
  SELECT
    t.ticket_id,
    COALESCE(s.comment_count, 0)::INT AS comment_count,
    COALESCE(s.public_comment_count, 0)::INT AS public_comment_count,
    COALESCE(s.agent_participants, ARRAY_CONSTRUCT())::ARRAY AS agent_participants,
    COALESCE(s.author_sequence, ARRAY_CONSTRUCT())::ARRAY AS author_sequence,
    COALESCE(s.handoff_count, 0)::INT AS handoff_count,
    OBJECT_CONSTRUCT_KEEP_NULL(
      'agent',
      s.agent_comments,
      'customer',
      s.customer_comments,
      'system',
      NULLIF(s.system_comments, 0)
    )::OBJECT AS author_breakdown,
    COALESCE(s.total_attachments, 0)::INT AS total_attachments,
    s.last_comment_at,
    s.last_author_type::VARCHAR AS last_author_type,
    COALESCE(s.mentions_order, FALSE)::BOOLEAN AS mentions_order
  FROM tickets AS t
  LEFT JOIN thread_stats AS s
    ON t.ticket_id = s.ticket_id
)

SELECT *
FROM final
