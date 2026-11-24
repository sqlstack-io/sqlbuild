MODEL (
  description "One row per live survey response with the net promoter answer extracted",
  columns (
    response_id (type VARCHAR, nullable false, audits [not_null, unique]),
    survey_code (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    submitted_at (type TIMESTAMP_TZ),
    nps_score (type INT),
    answer_count (type INT),
    answers (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__survey_responses")
),

@sql.cdc_latest(src, src.key := (response_id))

final AS (
  SELECT
    r.response_id,
    UPPER(r.survey_code)::VARCHAR AS survey_code,
    r.customer_id,
    r.submitted_at,
    IFF(
      TRY_TO_NUMBER(r.answers:nps::VARCHAR) BETWEEN 0 AND 10,
      TRY_TO_NUMBER(r.answers:nps::VARCHAR),
      NULL
    )::INT AS nps_score,
    ARRAY_SIZE(OBJECT_KEYS(r.answers))::INT AS answer_count,
    r.answers::OBJECT AS answers,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
