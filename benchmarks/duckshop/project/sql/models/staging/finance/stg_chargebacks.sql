MODEL (
  description "One row per live card chargeback with the evidence submitted",
  columns (
    chargeback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    payment_id (type VARCHAR, audits [not_null]),
    amount_minor (type INT),
    currency_code (type VARCHAR),
    reason_code (type VARCHAR),
    reason_family (type VARCHAR),
    opened_at (type TIMESTAMP_TZ),
    closed_at (type TIMESTAMP_TZ),
    outcome (type VARCHAR),
    evidence_document_count (type INT),
    evidence (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("finance__chargebacks")
),

@sql.cdc_latest(src, src.key := (chargeback_id))

final AS (
  SELECT
    c.chargeback_id,
    c.payment_id,
    c.amount_minor::INT AS amount_minor,
    UPPER(c.currency_code)::VARCHAR AS currency_code,
    UPPER(c.reason_code)::VARCHAR AS reason_code,
    CASE
      WHEN REGEXP_LIKE(c.reason_code, '^(10|4837|4863).*')
      THEN 'fraud'
      WHEN REGEXP_LIKE(c.reason_code, '^(13|4853|4855).*')
      THEN 'consumer_dispute'
      ELSE 'processing'
    END::VARCHAR AS reason_family,
    c.opened_at,
    c.closed_at,
    LOWER(COALESCE(c.outcome, 'open'))::VARCHAR AS outcome,
    ARRAY_SIZE(COALESCE(c.evidence:documents, ARRAY_CONSTRUCT()))::INT AS evidence_document_count,
    c.evidence::OBJECT AS evidence,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
