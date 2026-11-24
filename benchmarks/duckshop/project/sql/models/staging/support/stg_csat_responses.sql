MODEL (
  description "One row per live customer satisfaction response",
  columns (
    response_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR, audits [not_null]),
    score (type INT, audits [accepted_values (values [1, 2, 3, 4, 5])]),
    is_satisfied (type BOOLEAN),
    comment_text (type VARCHAR),
    responded_at (type TIMESTAMP_TZ),
    effort_score (type INT),
    answers (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__csat_responses")
),

@sql.cdc_latest(src, src.key := (response_id))

final AS (
  SELECT
    r.response_id,
    r.ticket_id,
    r.score::INT AS score,
    (
      r.score >= 4
    ) AS is_satisfied,
    NULLIF(TRIM(r.comment), '')::VARCHAR AS comment_text,
    r.responded_at,
    TRY_TO_NUMBER(r.answers:effort::VARCHAR)::INT AS effort_score,
    COALESCE(r.answers, OBJECT_CONSTRUCT())::OBJECT AS answers,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
