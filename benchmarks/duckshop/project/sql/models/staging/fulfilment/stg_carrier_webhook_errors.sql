MODEL (
  description "One row per carrier webhook delivery whose body is not valid JSON, kept for replay",
  columns (
    webhook_id (type VARCHAR, nullable false, audits [not_null, unique]),
    carrier_code (type VARCHAR),
    received_at (type TIMESTAMP_NTZ),
    body_length (type INT),
    body_prefix (type VARCHAR),
    looks_truncated (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__carrier_webhooks")
),

latest AS (
  SELECT
    w.webhook_id,
    w.carrier_code,
    w.received_at,
    w.body
  FROM src AS w
  QUALIFY ROW_NUMBER() OVER (PARTITION BY w.webhook_id ORDER BY w._loaded_at DESC) = 1
),

final AS (
  SELECT
    l.webhook_id,
    UPPER(l.carrier_code)::VARCHAR AS carrier_code,
    l.received_at,
    LENGTH(l.body)::INT AS body_length,
    LEFT(REGEXP_REPLACE(l.body, '\\s+', ' '), 80)::VARCHAR AS body_prefix,
    (
      LEFT(LTRIM(l.body), 1) = '{'
      AND RIGHT(RTRIM(l.body), 1) <> '}'
    ) AS looks_truncated
  FROM latest AS l
  WHERE TRY_PARSE_JSON(l.body) IS NULL
)

SELECT *
FROM final
