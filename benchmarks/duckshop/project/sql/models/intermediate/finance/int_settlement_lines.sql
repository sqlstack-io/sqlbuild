MODEL (
  description """
    One row per line of each provider settlement report, flattened from the report JSON, with the
    payment reference parsed from free-text descriptions when the structured field is missing, and
    the line matched to a known payment or refund.
  """,
  columns (
    settlement_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    report_id (type VARCHAR, audits [not_null]),
    provider_code (type VARCHAR),
    report_date (type DATE),
    line_number (type INT),
    line_type (type VARCHAR),
    payment_reference (type VARCHAR),
    reference_source (type VARCHAR),
    gross_minor (type INT),
    fee_minor (type INT),
    net_minor (type INT),
    matched_payment_id (type VARCHAR),
    matched_refund_id (type VARCHAR),
    is_matched (type BOOLEAN),
    amount_matches (type BOOLEAN),
    fee_rate (type NUMBER(9, 6)),
  ),
);

WITH reports AS (
  SELECT *
  FROM __ref("stg_settlement_reports")
),

payments AS (
  SELECT *
  FROM __ref("stg_payments")
),

refunds AS (
  SELECT *
  FROM __ref("stg_refunds")
),

raw_lines AS (
  SELECT
    r.report_id,
    r.provider_code,
    r.report_date,
    l.index AS line_index,
    l.value AS line
  FROM reports AS r, LATERAL FLATTEN(input => r.report:lines) AS l
),

typed_lines AS (
  SELECT
    l.report_id,
    l.provider_code,
    l.report_date,
    l.line_index,
    LOWER(COALESCE(l.line:type::VARCHAR, l.line:record_type::VARCHAR, 'unknown')) AS line_type,
    NULLIF(
      TRIM(l.line:reference::VARCHAR),
      ''
    ) AS structured_reference,
    REGEXP_SUBSTR(
      l.line:description::VARCHAR,
      '(pay|pi|ch)_[A-Za-z0-9]{6,}'
    ) AS described_reference,
    COALESCE(
      l.line:gross_minor::INT,
      TRY_TO_NUMBER(l.line:gross::VARCHAR, 18, 2) * 100
    ) AS gross_minor,
    COALESCE(l.line:fee_minor::INT, 0) AS fee_minor
  FROM raw_lines AS l
),

referenced AS (
  SELECT
    t.report_id,
    t.provider_code,
    t.report_date,
    t.line_index,
    t.line_type,
    COALESCE(t.structured_reference, t.described_reference) AS payment_reference,
    CASE
      WHEN t.structured_reference IS NOT NULL
      THEN 'structured'
      WHEN t.described_reference IS NOT NULL
      THEN 'description'
      ELSE 'none'
    END AS reference_source,
    t.gross_minor,
    t.fee_minor
  FROM typed_lines AS t
),

payment_refs AS (
  SELECT
    p.gateway_reference::VARCHAR AS payment_reference,
    MIN(p.payment_id) AS payment_id,
    MIN(p.amount_minor) AS payment_amount_minor
  FROM payments AS p
  WHERE p.gateway_reference IS NOT NULL
  GROUP BY
    p.gateway_reference::VARCHAR
),

refund_refs AS (
  SELECT
    f.refund_id AS refund_reference,
    MIN(f.amount_minor) AS refund_amount_minor
  FROM refunds AS f
  GROUP BY
    f.refund_id
),

matched AS (
  SELECT
    r.report_id,
    r.provider_code,
    r.report_date,
    r.line_index,
    r.line_type,
    r.payment_reference,
    r.reference_source,
    r.gross_minor,
    r.fee_minor,
    p.payment_id AS matched_payment_id,
    f.refund_reference AS matched_refund_id,
    COALESCE(p.payment_amount_minor, f.refund_amount_minor) AS matched_amount_minor
  FROM referenced AS r
  LEFT JOIN payment_refs AS p
    ON r.payment_reference = p.payment_reference
  LEFT JOIN refund_refs AS f
    ON r.payment_reference = f.refund_reference
),

final AS (
  SELECT
    (
      m.report_id || ':' || LPAD(m.line_index::VARCHAR, 5, '0')
    )::VARCHAR AS settlement_line_key,
    m.report_id,
    m.provider_code,
    m.report_date,
    (
      m.line_index + 1
    )::INT AS line_number,
    m.line_type::VARCHAR AS line_type,
    m.payment_reference::VARCHAR AS payment_reference,
    m.reference_source::VARCHAR AS reference_source,
    m.gross_minor::INT AS gross_minor,
    m.fee_minor::INT AS fee_minor,
    (
      m.gross_minor - m.fee_minor
    )::INT AS net_minor,
    m.matched_payment_id::VARCHAR AS matched_payment_id,
    m.matched_refund_id::VARCHAR AS matched_refund_id,
    (
      m.matched_payment_id IS NOT NULL
      OR m.matched_refund_id IS NOT NULL
    )::BOOLEAN AS is_matched,
    COALESCE(ABS(m.gross_minor) = ABS(m.matched_amount_minor), FALSE)::BOOLEAN AS amount_matches,
    ROUND(m.fee_minor / NULLIF(ABS(m.gross_minor), 0), 6)::NUMBER(9, 6) AS fee_rate
  FROM matched AS m
)

SELECT *
FROM final
