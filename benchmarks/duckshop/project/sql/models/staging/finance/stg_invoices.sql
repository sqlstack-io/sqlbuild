MODEL (
  description "One row per live customer invoice with totals in minor units",
  columns (
    invoice_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    invoice_number (type VARCHAR, audits [unique]),
    invoice_series (type VARCHAR),
    issued_at (type TIMESTAMP_TZ),
    currency_code (type VARCHAR),
    net_minor (type INT),
    tax_minor (type INT),
    gross_minor (type INT),
    tax_lines (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("finance__invoices")
),

@sql.cdc_latest(src, src.key := (invoice_id))

final AS (
  SELECT
    i.invoice_id,
    i.order_id,
    UPPER(i.invoice_number)::VARCHAR AS invoice_number,
    REGEXP_SUBSTR(
      UPPER(i.invoice_number),
      '^([A-Z]{2,4})-',
      1,
      1,
      'e',
      1
    )::VARCHAR AS invoice_series,
    i.issued_at,
    UPPER(i.currency_code)::VARCHAR AS currency_code,
    i.totals:net_minor::INT AS net_minor,
    i.totals:tax_minor::INT AS tax_minor,
    COALESCE(
      i.totals:gross_minor::INT,
      i.totals:net_minor::INT + i.totals:tax_minor::INT
    )::INT AS gross_minor,
    COALESCE(i.tax_lines, ARRAY_CONSTRUCT())::ARRAY AS tax_lines,
    @sql.load_metadata(i)
  FROM cdc_latest AS i
)
SELECT *
FROM final
