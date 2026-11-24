MODEL (
  description "One row per live general ledger posting with a signed amount",
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    journal_code (type VARCHAR),
    account_code (type VARCHAR, audits [not_null]),
    account_class (type VARCHAR),
    signed_amount_minor (type INT),
    currency_code (type VARCHAR),
    posted_at (type TIMESTAMP_TZ, audits [not_null]),
    reference_type (type VARCHAR),
    reference_id (type VARCHAR),
    reference (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("finance__ledger_entries")
),

@sql.cdc_latest(src, src.key := (entry_id))

final AS (
  SELECT
    e.entry_id,
    UPPER(e.journal_code)::VARCHAR AS journal_code,
    e.account_code,
    CASE LEFT(e.account_code, 1)
      WHEN '1'
      THEN 'asset'
      WHEN '2'
      THEN 'liability'
      WHEN '3'
      THEN 'equity'
      WHEN '4'
      THEN 'revenue'
      ELSE 'expense'
    END::VARCHAR AS account_class,
    (
      COALESCE(e.debit_minor, 0) - COALESCE(e.credit_minor, 0)
    )::INT AS signed_amount_minor,
    UPPER(e.currency_code)::VARCHAR AS currency_code,
    e.posted_at,
    LOWER(e.reference:type::VARCHAR)::VARCHAR AS reference_type,
    @sql.json_text(e.reference:id) AS reference_id,
    e.reference::OBJECT AS reference,
    @sql.load_metadata(e)
  FROM cdc_latest AS e
)
SELECT *
FROM final
