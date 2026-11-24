MODEL (
  description "One row per tax line on an invoice, flattened from the invoice tax lines",
  columns (
    invoice_tax_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    invoice_id (type VARCHAR, audits [not_null]),
    tax_code (type VARCHAR),
    tax_rate_pct (type NUMBER(9, 4)),
    taxable_minor (type INT),
    tax_minor (type INT),
  ),
);

WITH invoices AS (
  SELECT *
  FROM __ref("stg_invoices")
),

final AS (
  SELECT
    (
      i.invoice_id || ':' || t.index::VARCHAR
    )::VARCHAR AS invoice_tax_line_key,
    i.invoice_id,
    UPPER(t.value:code::VARCHAR)::VARCHAR AS tax_code,
    t.value:rate_pct::NUMBER(9, 4) AS tax_rate_pct,
    t.value:taxable_minor::INT AS taxable_minor,
    t.value:tax_minor::INT AS tax_minor
  FROM invoices AS i, LATERAL FLATTEN(input => i.tax_lines) AS t
)

SELECT *
FROM final
