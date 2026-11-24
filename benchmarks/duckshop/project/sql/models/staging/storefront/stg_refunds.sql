MODEL (
  description "One row per live refund with a coarse reason category parsed from free text",
  columns (
    refund_id (type VARCHAR, nullable false, audits [not_null, unique]),
    payment_id (type VARCHAR, audits [not_null]),
    amount_minor (type INT, audits [not_null]),
    reason_text (type VARCHAR),
    reason_category (type VARCHAR),
    refunded_at (type TIMESTAMP_TZ),
    initiated_by (type VARCHAR),
    refunded_line_count (type INT),
    refunded_items (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__refunds")
),

@sql.cdc_latest(src, src.key := (refund_id))

final AS (
  SELECT
    r.refund_id,
    r.payment_id,
    ABS(r.amount_minor)::INT AS amount_minor,
    TRIM(r.reason_text)::VARCHAR AS reason_text,
    CASE
      WHEN REGEXP_LIKE(r.reason_text, '.*(damag|broken|defect).*', 'i')
      THEN 'damaged'
      WHEN REGEXP_LIKE(r.reason_text, '.*(late|delay|never arrived).*', 'i')
      THEN 'delivery'
      WHEN REGEXP_LIKE(r.reason_text, '.*(fraud|chargeback).*', 'i')
      THEN 'fraud'
      ELSE 'other'
    END::VARCHAR AS reason_category,
    r.refunded_at,
    LOWER(COALESCE(r.initiated_by, 'agent'))::VARCHAR AS initiated_by,
    ARRAY_SIZE(COALESCE(r.items, ARRAY_CONSTRUCT()))::INT AS refunded_line_count,
    COALESCE(r.items, ARRAY_CONSTRUCT())::ARRAY AS refunded_items,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
