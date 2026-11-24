MODEL (
  description """
    One row per answered question in each customer's latest response to each survey, with the answer
    typed by its JSON kind, free-text answers cleaned, and the response's NPS group. JSON null
    answers are kept as unanswered rows; missing keys produce no row.
  """,
  columns (
    answer_key (type VARCHAR, nullable false, audits [not_null, unique]),
    response_id (type VARCHAR, audits [not_null]),
    survey_code (type VARCHAR),
    customer_id (type VARCHAR),
    submitted_at (type TIMESTAMP_TZ),
    question_key (type VARCHAR),
    answer_kind (type VARCHAR),
    answer_number (type NUMBER(18, 4)),
    answer_flag (type BOOLEAN),
    answer_text (type VARCHAR),
    answer_choices (type ARRAY),
    is_answered (type BOOLEAN),
    nps_group (type VARCHAR),
    question_position (type INT),
  ),
);

WITH responses AS (
  SELECT *
  FROM __ref("stg_survey_responses")
),

latest_responses AS (
  SELECT
    r.response_id,
    r.survey_code,
    r.customer_id,
    r.submitted_at,
    r.nps_score,
    r.answers
  FROM responses AS r
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY r.customer_id, r.survey_code
      ORDER BY r.submitted_at DESC NULLS LAST, r.response_id DESC
    ) = 1
),

answers AS (
  SELECT
    r.response_id,
    r.survey_code,
    r.customer_id,
    r.submitted_at,
    r.nps_score,
    a.key AS raw_question_key,
    a.value AS answer_value,
    TYPEOF(a.value) AS value_type
  FROM latest_responses AS r, LATERAL FLATTEN(input => r.answers) AS a
),

typed AS (
  SELECT
    a.response_id,
    a.survey_code,
    a.customer_id,
    a.submitted_at,
    a.nps_score,
    LOWER(REGEXP_REPLACE(TRIM(a.raw_question_key), '[^A-Za-z0-9]+', '_')) AS question_key,
    CASE a.value_type
      WHEN 'NULL_VALUE'
      THEN 'null'
      WHEN 'INTEGER'
      THEN 'number'
      WHEN 'DECIMAL'
      THEN 'number'
      WHEN 'DOUBLE'
      THEN 'number'
      WHEN 'BOOLEAN'
      THEN 'flag'
      WHEN 'ARRAY'
      THEN 'choices'
      WHEN 'VARCHAR'
      THEN IFF(TRY_TO_NUMBER(a.answer_value::VARCHAR, 18, 4) IS NULL, 'text', 'number')
      ELSE 'other'
    END AS answer_kind,
    a.answer_value
  FROM answers AS a
),

cleaned AS (
  SELECT
    t.response_id,
    t.survey_code,
    t.customer_id,
    t.submitted_at,
    t.nps_score,
    t.question_key,
    t.answer_kind,
    IFF(
      t.answer_kind = 'number',
      TRY_TO_NUMBER(t.answer_value::VARCHAR, 18, 4),
      NULL
    ) AS answer_number,
    IFF(t.answer_kind = 'flag', t.answer_value::BOOLEAN, NULL) AS answer_flag,
    IFF(
      t.answer_kind = 'text',
      REGEXP_REPLACE(@sql.any_to_text(t.answer_value), '\\s{2,}', ' '),
      NULL
    ) AS answer_text,
    IFF(t.answer_kind = 'choices', t.answer_value::ARRAY, NULL) AS answer_choices
  FROM typed AS t
),

final AS (
  SELECT
    (
      c.response_id || ':' || c.question_key
    )::VARCHAR AS answer_key,
    c.response_id,
    c.survey_code,
    c.customer_id,
    c.submitted_at,
    c.question_key::VARCHAR AS question_key,
    c.answer_kind::VARCHAR AS answer_kind,
    c.answer_number::NUMBER(18, 4) AS answer_number,
    c.answer_flag::BOOLEAN AS answer_flag,
    c.answer_text::VARCHAR AS answer_text,
    c.answer_choices::ARRAY AS answer_choices,
    (
      c.answer_kind <> 'null'
    )::BOOLEAN AS is_answered,
    CASE
      WHEN c.nps_score IS NULL
      THEN NULL
      WHEN c.nps_score >= 9
      THEN 'promoter'
      WHEN c.nps_score >= 7
      THEN 'passive'
      ELSE 'detractor'
    END::VARCHAR AS nps_group,
    RANK() OVER (PARTITION BY c.response_id ORDER BY c.question_key)::INT AS question_position
  FROM cleaned AS c
)

SELECT *
FROM final
