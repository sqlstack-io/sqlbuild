MODEL (
  description "One row per live customer return request",
  columns (
    return_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    qty (type INT),
    reason_text (type VARCHAR),
    return_status (type VARCHAR),
    requested_at (type TIMESTAMP_TZ),
    received_at (type TIMESTAMP_TZ),
    photo_count (type INT),
    refund_preference (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("fulfilment__returns")
),

@sql.cdc_latest(src, src.key := (return_id))

final AS (
  SELECT
    r.return_id,
    r.order_id,
    @sql.normalise_sku(r.sku) AS sku,
    GREATEST(r.qty, 1)::INT AS qty,
    REGEXP_REPLACE(TRIM(r.reason_text), '\\s+', ' ')::VARCHAR AS reason_text,
    LOWER(r.return_status)::VARCHAR AS return_status,
    r.requested_at,
    r.received_at,
    ARRAY_SIZE(COALESCE(r.photos, ARRAY_CONSTRUCT()))::INT AS photo_count,
    LOWER(
      COALESCE(r.refund_preference:method::VARCHAR, 'original_payment')
    )::VARCHAR AS refund_preference,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
