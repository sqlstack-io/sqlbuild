MODEL (
  description """
    One row per ship-to country and quarter with tax lines checked, rate mismatches and net variance
  """,
  columns (
    tax_variance_key (type VARCHAR, nullable false, audits [not_null, unique]),
    ship_to_country_code (type VARCHAR),
    issued_quarter (type DATE),
    tax_lines (type INT),
    lines_without_rate (type INT),
    rate_mismatches (type INT),
    charged_tax_minor (type INT),
    net_variance_minor (type INT),
    absolute_variance_minor (type INT),
    ambiguous_rate_lines (type INT),
  ),
);

WITH tax_lines AS (
  SELECT *
  FROM __ref("int_tax_applied")
),

final AS (
  SELECT
    (
      COALESCE(t.ship_to_country_code, '??')
        || ':'
        || TO_VARCHAR(DATE_TRUNC('quarter', t.issued_date), 'YYYY-MM-DD')
    )::VARCHAR AS tax_variance_key,
    COALESCE(t.ship_to_country_code, '??')::VARCHAR AS ship_to_country_code,
    DATE_TRUNC('quarter', t.issued_date)::DATE AS issued_quarter,
    COUNT(*)::INT AS tax_lines,
    COUNT_IF(NOT t.has_published_rate)::INT AS lines_without_rate,
    COUNT_IF(t.is_rate_mismatch)::INT AS rate_mismatches,
    SUM(t.charged_tax_minor)::INT AS charged_tax_minor,
    SUM(t.variance_minor)::INT AS net_variance_minor,
    SUM(ABS(t.variance_minor))::INT AS absolute_variance_minor,
    COUNT_IF(t.candidate_rate_count > 1)::INT AS ambiguous_rate_lines
  FROM tax_lines AS t
  GROUP BY
    COALESCE(t.ship_to_country_code, '??'),
    DATE_TRUNC('quarter', t.issued_date)
)

SELECT *
FROM final
