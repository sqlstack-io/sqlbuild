MODEL (
  description "One row per visitor and checkout step reached on web, earliest occurrence only",
  columns (
    checkout_step_key (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR, audits [not_null]),
    step_number (type INT, audits [not_null]),
    step_name (type VARCHAR),
    first_reached_at_utc (type TIMESTAMP_NTZ),
    payment_option (type VARCHAR),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

steps AS (
  SELECT
    e.event_id,
    e.anonymous_id,
    e.event_at_utc,
    COALESCE(
      e.properties:step::INT,
      TRY_TO_NUMBER(REGEXP_SUBSTR(e.page_path, '/checkout/step-([0-9])', 1, 1, 'e', 1))
    ) AS step_number,
    LOWER(e.properties:step_name::VARCHAR) AS step_name,
    LOWER(e.properties:payment_option::VARCHAR) AS payment_option
  FROM web_events AS e
  WHERE
    e.event_type IN ('checkout_step', 'checkout_started')
    AND e.anonymous_id IS NOT NULL
),

final AS (
  SELECT
    (
      s.anonymous_id || '#' || s.step_number::VARCHAR
    )::VARCHAR AS checkout_step_key,
    s.anonymous_id,
    s.step_number::INT AS step_number,
    s.step_name::VARCHAR AS step_name,
    s.event_at_utc AS first_reached_at_utc,
    s.payment_option::VARCHAR AS payment_option
  FROM steps AS s
  WHERE s.step_number IS NOT NULL
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY s.anonymous_id, s.step_number
      ORDER BY s.event_at_utc, s.event_id
    ) = 1
)

SELECT *
FROM final
