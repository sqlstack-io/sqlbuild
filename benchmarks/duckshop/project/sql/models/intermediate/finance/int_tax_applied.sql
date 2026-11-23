MODEL (
  description """
    One row per invoice tax line with the published tax rate in force for the ship-to country and
    tax category on the issue date, the tax that rate implies, and the variance from the tax
    charged.
  """,
  columns (
    invoice_tax_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    invoice_id (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    tax_code (type VARCHAR),
    tax_category (type VARCHAR),
    ship_to_country_code (type VARCHAR),
    issued_date (type DATE),
    charged_rate_pct (type NUMBER(9, 4)),
    published_rate_pct (type NUMBER(9, 4)),
    taxable_minor (type INT),
    charged_tax_minor (type INT),
    expected_tax_minor (type INT),
    variance_minor (type INT),
    has_published_rate (type BOOLEAN),
    is_rate_mismatch (type BOOLEAN),
    candidate_rate_count (type INT),
  ),
);

WITH tax_lines AS (
  SELECT *
  FROM __ref("stg_invoice_tax_lines")
),

invoices AS (
  SELECT *
  FROM __ref("stg_invoices")
),

orders AS (
  SELECT *
  FROM __ref("int_orders_enriched")
),

tax_rates AS (
  SELECT *
  FROM __ref("stg_tax_rates")
),

rate_windows AS (
  SELECT
    r.tax_rate_id,
    r.country_code,
    LOWER(r.tax_category) AS tax_category,
    r.rate_pct,
    r.valid_from,
    COALESCE(r.valid_to, '9999-12-31'::DATE) AS valid_to_effective
  FROM tax_rates AS r
),

line_context AS (
  SELECT
    l.invoice_tax_line_key,
    l.invoice_id,
    i.order_id,
    UPPER(l.tax_code) AS tax_code,
    LOWER(
      COALESCE(REGEXP_SUBSTR(l.tax_code, '^[A-Za-z]+[_-]([A-Za-z]+)', 1, 1, 'e', 1), 'standard')
    ) AS tax_category,
    CONVERT_TIMEZONE('UTC', i.issued_at)::DATE AS issued_date,
    l.tax_rate_pct AS charged_rate_pct,
    l.taxable_minor,
    l.tax_minor AS charged_tax_minor
  FROM tax_lines AS l
  INNER JOIN invoices AS i
    ON l.invoice_id = i.invoice_id
),

with_country AS (
  SELECT
    c.invoice_tax_line_key,
    c.invoice_id,
    c.order_id,
    c.tax_code,
    c.tax_category,
    o.ship_to_country_code,
    c.issued_date,
    c.charged_rate_pct,
    c.taxable_minor,
    c.charged_tax_minor
  FROM line_context AS c
  LEFT JOIN orders AS o
    ON c.order_id = o.order_id
),

candidate_rates AS (
  SELECT
    w.invoice_tax_line_key,
    MAX_BY(r.rate_pct, r.valid_from) AS published_rate_pct,
    COUNT(r.tax_rate_id) AS candidate_rate_count
  FROM with_country AS w
  INNER JOIN rate_windows AS r
    ON w.ship_to_country_code = r.country_code
    AND w.tax_category = r.tax_category
    AND w.issued_date >= r.valid_from
    AND w.issued_date < r.valid_to_effective
  GROUP BY
    w.invoice_tax_line_key
),

expected AS (
  SELECT
    w.invoice_tax_line_key,
    w.invoice_id,
    w.order_id,
    w.tax_code,
    w.tax_category,
    w.ship_to_country_code,
    w.issued_date,
    w.charged_rate_pct,
    c.published_rate_pct,
    w.taxable_minor,
    w.charged_tax_minor,
    ROUND(w.taxable_minor * c.published_rate_pct / 100, 0) AS expected_tax_minor,
    COALESCE(c.candidate_rate_count, 0) AS candidate_rate_count
  FROM with_country AS w
  LEFT JOIN candidate_rates AS c
    ON w.invoice_tax_line_key = c.invoice_tax_line_key
),

final AS (
  SELECT
    e.invoice_tax_line_key,
    e.invoice_id,
    e.order_id,
    e.tax_code::VARCHAR AS tax_code,
    e.tax_category::VARCHAR AS tax_category,
    e.ship_to_country_code,
    e.issued_date,
    e.charged_rate_pct,
    e.published_rate_pct::NUMBER(9, 4) AS published_rate_pct,
    e.taxable_minor,
    e.charged_tax_minor,
    e.expected_tax_minor::INT AS expected_tax_minor,
    (
      e.charged_tax_minor - e.expected_tax_minor
    )::INT AS variance_minor,
    (
      e.published_rate_pct IS NOT NULL
    )::BOOLEAN AS has_published_rate,
    COALESCE(e.charged_rate_pct <> e.published_rate_pct, FALSE)::BOOLEAN AS is_rate_mismatch,
    e.candidate_rate_count::INT AS candidate_rate_count
  FROM expected AS e
)

SELECT *
FROM final
