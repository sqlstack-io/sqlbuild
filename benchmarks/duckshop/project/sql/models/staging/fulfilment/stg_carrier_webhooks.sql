MODEL (
  description """
    One row per carrier status update from webhook bodies that parse as JSON. Some carriers send a
    single update per delivery and others batch several updates in an events array.
  """,
  columns (
    carrier_update_key (type VARCHAR, nullable false, audits [not_null, unique]),
    webhook_id (type VARCHAR, audits [not_null]),
    carrier_code (type VARCHAR, audits [not_null]),
    received_at (type TIMESTAMP_NTZ),
    tracking_number (type VARCHAR),
    status_code (type VARCHAR),
    status_at (type TIMESTAMP_TZ),
    carrier_event_id (type NUMBER(38, 0)),
    is_batched (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__carrier_webhooks")
),

parsed AS (
  SELECT
    w.webhook_id,
    w.carrier_code,
    w.received_at,
    TRY_PARSE_JSON(w.body) AS payload
  FROM src AS w
  QUALIFY ROW_NUMBER() OVER (PARTITION BY w.webhook_id ORDER BY w._loaded_at DESC) = 1
),

single_updates AS (
  SELECT
    p.webhook_id,
    0 AS update_index,
    p.carrier_code,
    p.received_at,
    p.payload AS update_payload,
    FALSE AS is_batched
  FROM parsed AS p
  WHERE
    p.payload IS NOT NULL
    AND NOT COALESCE(IS_ARRAY(p.payload:events), FALSE)
),

batched_updates AS (
  SELECT
    p.webhook_id,
    e.index AS update_index,
    p.carrier_code,
    p.received_at,
    e.value AS update_payload,
    TRUE AS is_batched
  FROM parsed AS p, LATERAL FLATTEN(input => p.payload:events) AS e
  WHERE IS_ARRAY(p.payload:events)
),

all_updates AS (
  SELECT
    s.webhook_id,
    s.update_index,
    s.carrier_code,
    s.received_at,
    s.update_payload,
    s.is_batched
  FROM single_updates AS s
  UNION ALL
  SELECT
    b.webhook_id,
    b.update_index,
    b.carrier_code,
    b.received_at,
    b.update_payload,
    b.is_batched
  FROM batched_updates AS b
),

final AS (
  SELECT
    (
      u.webhook_id || ':' || u.update_index::VARCHAR
    )::VARCHAR AS carrier_update_key,
    u.webhook_id,
    UPPER(u.carrier_code)::VARCHAR AS carrier_code,
    u.received_at,
    UPPER(REGEXP_REPLACE(
      COALESCE(u.update_payload:tracking_number, u.update_payload:trackingNumber)::VARCHAR,
      '[^A-Za-z0-9]',
      ''
    ))::VARCHAR AS tracking_number,
    UPPER(
      COALESCE(u.update_payload:status.code, u.update_payload:event)::VARCHAR
    )::VARCHAR AS status_code,
    TRY_TO_TIMESTAMP_TZ(
      COALESCE(u.update_payload:status.at, u.update_payload:timestamp)::VARCHAR
    ) AS status_at,
    TRY_TO_NUMBER(u.update_payload:event_id::VARCHAR)::NUMBER(38, 0) AS carrier_event_id,
    u.is_batched
  FROM all_updates AS u
)

SELECT *
FROM final
