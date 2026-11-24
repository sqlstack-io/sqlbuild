MODEL (
  description """
    One row per partner and settlement quarter with stated versus expected commission and statements
    out of tolerance
  """,
  columns (
    partner_quarter_key (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR),
    settlement_quarter (type DATE),
    statements (type INT),
    gross_sales_usd (type NUMBER(18, 2)),
    stated_commission_usd (type NUMBER(18, 2)),
    expected_commission_usd (type NUMBER(18, 2)),
    variance_usd (type NUMBER(18, 2)),
    statements_out_of_tolerance (type INT),
    unbalanced_statements (type INT),
  ),
);

WITH fees AS (
  SELECT *
  FROM __ref("int_partner_fees_usd")
),

final AS (
  SELECT
    (
      f.partner_code || ':' || TO_VARCHAR(DATE_TRUNC('quarter', f.period_end), 'YYYY-MM-DD')
    )::VARCHAR AS partner_quarter_key,
    f.partner_code,
    DATE_TRUNC('quarter', f.period_end)::DATE AS settlement_quarter,
    COUNT(*)::INT AS statements,
    SUM(f.gross_sales_usd)::NUMBER(18, 2) AS gross_sales_usd,
    SUM(f.stated_commission_usd)::NUMBER(18, 2) AS stated_commission_usd,
    SUM(f.expected_commission_usd)::NUMBER(18, 2) AS expected_commission_usd,
    (
      SUM(f.stated_commission_usd) - SUM(f.expected_commission_usd)
    )::NUMBER(18, 2) AS variance_usd,
    COUNT_IF(NOT f.is_within_tolerance)::INT AS statements_out_of_tolerance,
    COUNT_IF(NOT f.statement_balances)::INT AS unbalanced_statements
  FROM fees AS f
  GROUP BY
    f.partner_code,
    DATE_TRUNC('quarter', f.period_end)
)

SELECT *
FROM final
