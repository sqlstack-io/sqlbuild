MODEL (
  description """
    One row per live support ticket; created_at is a session-time-zone dependent TIMESTAMP_LTZ in
    the source, an expected divergence across engines
  """,
  tags [known_divergence],
  columns (
    ticket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_email (type VARCHAR),
    channel (type VARCHAR),
    priority (type INT, audits [accepted_values (values [1, 2, 3, 4])]),
    ticket_status (type VARCHAR),
    created_at (type TIMESTAMP_LTZ, audits [not_null]),
    solved_at (type TIMESTAMP_TZ),
    order_id (type VARCHAR),
    contact_reason (type VARCHAR),
    tag_list (type ARRAY),
    is_vip (type BOOLEAN),
    custom_fields (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__tickets")
),

@sql.cdc_latest(src, src.key := (ticket_id))

final AS (
  SELECT
    t.ticket_id,
    @sql.normalise_email(t.customer_email) AS customer_email,
    LOWER(t.channel)::VARCHAR AS channel,
    LEAST(GREATEST(COALESCE(t.priority, 3), 1), 4)::INT AS priority,
    LOWER(t.ticket_status)::VARCHAR AS ticket_status,
    t.created_at::TIMESTAMP_LTZ AS created_at,
    t.solved_at,
    COALESCE(@sql.json_text(t.custom_fields:order_id), REGEXP_SUBSTR(
      t.custom_fields:order_ref::VARCHAR,
      'DS-O-[0-9]+'
    ))::VARCHAR AS order_id,
    LOWER(@sql.json_text(GET_PATH(t.custom_fields, 'contact_reason')))::VARCHAR AS contact_reason,
    COALESCE(t.tags, ARRAY_CONSTRUCT())::ARRAY AS tag_list,
    ARRAY_CONTAINS('vip'::VARIANT, COALESCE(t.tags, ARRAY_CONSTRUCT()))::BOOLEAN AS is_vip,
    COALESCE(t.custom_fields, OBJECT_CONSTRUCT())::OBJECT AS custom_fields,
    @sql.load_metadata(t)
  FROM cdc_latest AS t
)
SELECT *
FROM final
