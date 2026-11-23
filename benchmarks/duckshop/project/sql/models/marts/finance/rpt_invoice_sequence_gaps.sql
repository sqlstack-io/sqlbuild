MODEL (
  description """
    One row per invoice series with issued invoices, numbering gaps and tax reconciliation failures
  """,
  columns (
    invoice_series (type VARCHAR, nullable false, audits [not_null, unique]),
    invoices (type INT),
    first_sequence (type INT),
    last_sequence (type INT),
    gap_count (type INT),
    missing_numbers (type INT),
    unbalanced_headers (type INT),
    tax_disagreements (type INT),
    currency_mismatches (type INT),
  ),
);

WITH invoices AS (
  SELECT *
  FROM __ref("int_invoices_reconciled")
),

final AS (
  SELECT
    i.invoice_series,
    COUNT(*)::INT AS invoices,
    MIN(i.invoice_sequence)::INT AS first_sequence,
    MAX(i.invoice_sequence)::INT AS last_sequence,
    COUNT(i.sequence_gap)::INT AS gap_count,
    COALESCE(SUM(i.sequence_gap), 0)::INT AS missing_numbers,
    COUNT_IF(NOT i.header_balances)::INT AS unbalanced_headers,
    COUNT_IF(NOT i.tax_lines_agree OR NOT i.matches_order_tax)::INT AS tax_disagreements,
    COUNT_IF(NOT i.invoice_currency_matches_order)::INT AS currency_mismatches
  FROM invoices AS i
  GROUP BY
    i.invoice_series
)

SELECT *
FROM final
