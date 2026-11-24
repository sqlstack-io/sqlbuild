MODEL (
  description """
    One row per survey and month with responses, NPS group counts and the net promoter score
  """,
  columns (
    nps_key (type VARCHAR, nullable false, audits [not_null, unique]),
    survey_code (type VARCHAR),
    response_month (type DATE),
    responses (type INT),
    promoters (type INT),
    passives (type INT),
    detractors (type INT),
    net_promoter_score (type NUMBER(6, 2)),
    free_text_answers (type INT),
  ),
);

WITH answers AS (
  SELECT *
  FROM __ref("int_survey_answers_flat")
),

responses AS (
  SELECT
    a.response_id,
    a.survey_code,
    DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', a.submitted_at))::DATE AS response_month,
    MAX(a.nps_group) AS nps_group,
    COUNT_IF(a.answer_kind = 'text') AS free_text_answers
  FROM answers AS a
  GROUP BY
    1,
    2,
    3
),

final AS (
  SELECT
    (
      r.survey_code || ':' || TO_VARCHAR(r.response_month, 'YYYY-MM')
    )::VARCHAR AS nps_key,
    r.survey_code,
    r.response_month,
    COUNT(DISTINCT r.response_id)::INT AS responses,
    COUNT_IF(r.nps_group = 'promoter')::INT AS promoters,
    COUNT_IF(r.nps_group = 'passive')::INT AS passives,
    COUNT_IF(r.nps_group = 'detractor')::INT AS detractors,
    ROUND(
      100
      * (
        COUNT_IF(r.nps_group = 'promoter') - COUNT_IF(r.nps_group = 'detractor')
      )
      / NULLIF(COUNT(r.nps_group), 0),
      2
    )::NUMBER(6, 2) AS net_promoter_score,
    SUM(r.free_text_answers)::INT AS free_text_answers
  FROM responses AS r
  GROUP BY
    r.survey_code,
    r.response_month
)

SELECT *
FROM final
