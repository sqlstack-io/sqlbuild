MODEL (
  description """
    One row per invoice with its number parsed into series and sequence, gaps in each series'
    numbering, and the invoice's tax reconciled three ways: the header total, the embedded tax-line
    array and the separately loaded tax lines, against the order's own tax.
  """,
  constants (
    _tax_tolerance_minor 1,
  ),
  columns (
    invoice_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    invoice_series (type VARCHAR),
    invoice_sequence (type INT),
    sequence_gap (type INT),
    issued_at (type TIMESTAMP_TZ),
    issued_date_local (type DATE),
    currency_code (type VARCHAR),
    net (type NUMBER(38, 6)),
    tax (type NUMBER(38, 6)),
    gross (type NUMBER(38, 6)),
    embedded_tax_minor (type INT),
    loaded_tax_minor (type INT),
    order_tax_minor (type INT),
    tax_line_count (type INT),
    tax_codes (type ARRAY),
    header_balances (type BOOLEAN),
    tax_lines_agree (type BOOLEAN),
    matches_order_tax (type BOOLEAN),
    invoice_currency_matches_order (type BOOLEAN),
  ),
);

WITH invoices AS (
  SELECT *
  FROM __ref("stg_invoices")
),

tax_lines AS (
  SELECT *
  FROM __ref("stg_invoice_tax_lines")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_enriched")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

numbered AS (
  SELECT
    i.invoice_id,
    i.order_id,
    COALESCE(
      i.invoice_series,
      REGEXP_SUBSTR(i.invoice_number, '^([A-Z]+)-', 1, 1, 'e', 1),
      'NONE'
    ) AS invoice_series,
    TRY_TO_NUMBER(REGEXP_SUBSTR(i.invoice_number, '([0-9]+)$', 1, 1, 'e', 1)) AS invoice_sequence,
    i.issued_at,
    i.currency_code,
    i.net_minor,
    i.tax_minor,
    i.gross_minor,
    i.tax_lines
  FROM invoices AS i
),

sequenced AS (
  SELECT
    n.invoice_id,
    n.invoice_sequence
      - LAG(n.invoice_sequence) OVER (
        PARTITION BY n.invoice_series
        ORDER BY n.invoice_sequence, n.invoice_id
      )
      - 1 AS sequence_gap
  FROM numbered AS n
),

embedded AS (
  SELECT
    n.invoice_id,
    SUM(t.value:tax_minor::INT) AS embedded_tax_minor,
    COUNT(t.value) AS embedded_line_count
  FROM numbered AS n, LATERAL FLATTEN(input => n.tax_lines) AS t
  GROUP BY
    n.invoice_id
),

loaded AS (
  SELECT
    l.invoice_id,
    SUM(l.tax_minor) AS loaded_tax_minor,
    COUNT(*) AS loaded_line_count,
    ARRAY_AGG(DISTINCT l.tax_code) WITHIN GROUP (ORDER BY l.tax_code) AS tax_codes
  FROM tax_lines AS l
  GROUP BY
    l.invoice_id
),

order_tax AS (
  SELECT
    o.order_id,
    o.tax_amount_minor,
    o.currency_code,
    o.storefront_timezone
  FROM orders AS o
),

joined AS (
  SELECT
    n.invoice_id,
    n.order_id,
    n.invoice_series,
    n.invoice_sequence,
    s.sequence_gap,
    n.issued_at,
    n.currency_code,
    n.net_minor,
    n.tax_minor,
    n.gross_minor,
    c.minor_unit_exponent,
    e.embedded_tax_minor,
    e.embedded_line_count,
    l.loaded_tax_minor,
    l.loaded_line_count,
    l.tax_codes,
    o.tax_amount_minor AS order_tax_minor,
    o.currency_code AS order_currency_code,
    o.storefront_timezone
  FROM numbered AS n
  INNER JOIN sequenced AS s
    ON n.invoice_id = s.invoice_id
  LEFT JOIN currencies AS c
    ON n.currency_code = c.currency_code
  LEFT JOIN embedded AS e
    ON n.invoice_id = e.invoice_id
  LEFT JOIN loaded AS l
    ON n.invoice_id = l.invoice_id
  LEFT JOIN order_tax AS o
    ON n.order_id = o.order_id
),

final AS (
  SELECT
    j.invoice_id,
    j.order_id,
    j.invoice_series::VARCHAR AS invoice_series,
    j.invoice_sequence::INT AS invoice_sequence,
    NULLIF(j.sequence_gap, 0)::INT AS sequence_gap,
    j.issued_at,
    @sql.local_business_date(j.issued_at, COALESCE(j.storefront_timezone, 'UTC')) AS issued_date_local,
    j.currency_code,
    @sql.money_from_minor(j.net_minor, COALESCE(j.minor_unit_exponent, 2)) AS net,
    @sql.money_from_minor(j.tax_minor, COALESCE(j.minor_unit_exponent, 2)) AS tax,
    @sql.money_from_minor(j.gross_minor, COALESCE(j.minor_unit_exponent, 2)) AS gross,
    j.embedded_tax_minor::INT AS embedded_tax_minor,
    j.loaded_tax_minor::INT AS loaded_tax_minor,
    j.order_tax_minor,
    GREATEST(
      COALESCE(j.embedded_line_count, 0),
      COALESCE(j.loaded_line_count, 0)
    )::INT AS tax_line_count,
    COALESCE(j.tax_codes, ARRAY_CONSTRUCT())::ARRAY AS tax_codes,
    (
      j.net_minor + j.tax_minor = j.gross_minor
    )::BOOLEAN AS header_balances,
    COALESCE(
      ABS(j.embedded_tax_minor - j.loaded_tax_minor) <= @const("_tax_tolerance_minor")
      AND ABS(j.loaded_tax_minor - j.tax_minor) <= @const("_tax_tolerance_minor"),
      FALSE
    )::BOOLEAN AS tax_lines_agree,
    COALESCE(
      ABS(j.tax_minor - j.order_tax_minor) <= @const("_tax_tolerance_minor"),
      FALSE
    )::BOOLEAN AS matches_order_tax,
    EQUAL_NULL(j.currency_code, j.order_currency_code)::BOOLEAN AS invoice_currency_matches_order
  FROM joined AS j
)

SELECT *
FROM final
