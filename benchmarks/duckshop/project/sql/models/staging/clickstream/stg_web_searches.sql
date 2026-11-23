MODEL (
  description "One row per on-site search event from the web tracker",
  columns (
    event_id (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR),
    searched_at_utc (type TIMESTAMP_NTZ),
    search_term (type VARCHAR),
    result_count (type INT),
    is_zero_result (type BOOLEAN),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

final AS (
  SELECT
    e.event_id,
    e.anonymous_id,
    e.event_at_utc AS searched_at_utc,
    LOWER(TRIM(REGEXP_REPLACE(
      e.properties:query::VARCHAR,
      '\\s+',
      ' '
    )))::VARCHAR AS search_term,
    e.properties:results::INT AS result_count,
    (
      COALESCE(e.properties:results::INT, 0) = 0
    ) AS is_zero_result
  FROM web_events AS e
  WHERE e.event_type = 'search'
)

SELECT *
FROM final
