MODEL (
  description """
    One row per partner settlement statement with the partner's stated commission compared with the
    commission implied by each order line in the period and the listing's agreed commission rate,
    all converted to USD at the rate on the period end.
  """,
  constants (
    _variance_tolerance_pct 0.5,
  ),
  columns (
    settlement_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    period_start (type DATE),
    period_end (type DATE),
    period_days (type INT),
    currency_code (type VARCHAR),
    gross_sales_usd (type NUMBER(18, 2)),
    stated_commission_usd (type NUMBER(18, 2)),
    expected_commission_usd (type NUMBER(18, 2)),
    other_fees_usd (type NUMBER(18, 2)),
    net_payable_usd (type NUMBER(18, 2)),
    period_order_count (type INT),
    lines_without_listing (type INT),
    effective_commission_pct (type NUMBER(9, 4)),
    commission_variance_pct (type NUMBER(9, 4)),
    is_within_tolerance (type BOOLEAN),
    statement_balances (type BOOLEAN),
  ),
);

WITH settlements AS (
  SELECT *
  FROM __ref("stg_partner_settlements")
),

orders AS (
  SELECT *
  FROM __ref("int_partner_orders_unified")
),

order_lines AS (
  SELECT *
  FROM __ref("stg_partner_order_lines")
),

listings AS (
  SELECT *
  FROM __ref("stg_partner_listings")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

listing_terms AS (
  SELECT
    l.partner_code,
    l.sku,
    l.commission_pct
  FROM listings AS l
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY l.partner_code, l.sku
      ORDER BY l.listed_at DESC NULLS LAST, l.listing_id DESC
    ) = 1
),

line_context AS (
  SELECT
    o.partner_code,
    o.placed_date,
    o.partner_order_key,
    l.sku,
    l.quantity * l.unit_amount_minor AS line_amount_minor
  FROM order_lines AS l
  INNER JOIN orders AS o
    ON l.partner_order_key = o.partner_order_key
),

line_commission AS (
  SELECT
    c.partner_code,
    c.placed_date,
    c.partner_order_key,
    c.line_amount_minor * t.commission_pct / 100 AS expected_commission_minor,
    (
      t.commission_pct IS NULL
    ) AS has_no_listing
  FROM line_context AS c
  LEFT JOIN listing_terms AS t
    ON c.partner_code = t.partner_code
    AND c.sku = t.sku
),

period_lines AS (
  SELECT
    s.settlement_id,
    SUM(c.expected_commission_minor) AS expected_commission_minor,
    COUNT(DISTINCT c.partner_order_key) AS period_order_count,
    COUNT_IF(c.has_no_listing) AS lines_without_listing
  FROM settlements AS s
  INNER JOIN line_commission AS c
    ON s.partner_code = c.partner_code
    AND c.placed_date BETWEEN s.period_start AND s.period_end
  GROUP BY
    s.settlement_id
),

with_rate AS (
  SELECT
    s.settlement_id,
    s.partner_code,
    s.period_start,
    s.period_end,
    s.currency_code,
    s.gross_sales_minor,
    s.commission_minor,
    s.other_fees_minor,
    s.net_payable_minor,
    cur.minor_unit_exponent,
    fx.rate_to_usd
  FROM settlements AS s
  LEFT JOIN currencies AS cur
    ON s.currency_code = cur.currency_code
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (s.period_end >= fx.rate_date)
    ON s.currency_code = fx.currency_code
),

converted AS (
  SELECT
    w.settlement_id,
    w.partner_code,
    w.period_start,
    w.period_end,
    w.currency_code,
    w.gross_sales_minor,
    w.commission_minor,
    w.other_fees_minor,
    w.net_payable_minor,
    @sql.money_from_minor(w.gross_sales_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) * w.rate_to_usd AS gross_sales_usd,
    @sql.money_from_minor(w.commission_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) * w.rate_to_usd AS stated_commission_usd,
    @sql.money_from_minor(p.expected_commission_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) * w.rate_to_usd AS expected_commission_usd,
    @sql.money_from_minor(w.other_fees_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) * w.rate_to_usd AS other_fees_usd,
    @sql.money_from_minor(w.net_payable_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) * w.rate_to_usd AS net_payable_usd,
    COALESCE(p.period_order_count, 0) AS period_order_count,
    COALESCE(p.lines_without_listing, 0) AS lines_without_listing
  FROM with_rate AS w
  LEFT JOIN period_lines AS p
    ON w.settlement_id = p.settlement_id
),

final AS (
  SELECT
    c.settlement_id,
    c.partner_code,
    c.period_start,
    c.period_end,
    (
      DATEDIFF('day', c.period_start, c.period_end) + 1
    )::INT AS period_days,
    c.currency_code,
    ROUND(c.gross_sales_usd, 2)::NUMBER(18, 2) AS gross_sales_usd,
    ROUND(c.stated_commission_usd, 2)::NUMBER(18, 2) AS stated_commission_usd,
    ROUND(c.expected_commission_usd, 2)::NUMBER(18, 2) AS expected_commission_usd,
    ROUND(c.other_fees_usd, 2)::NUMBER(18, 2) AS other_fees_usd,
    ROUND(c.net_payable_usd, 2)::NUMBER(18, 2) AS net_payable_usd,
    c.period_order_count::INT AS period_order_count,
    c.lines_without_listing::INT AS lines_without_listing,
    ROUND(
      100 * c.commission_minor / NULLIF(c.gross_sales_minor, 0),
      4
    )::NUMBER(9, 4) AS effective_commission_pct,
    ROUND(
      100
      * (
        c.stated_commission_usd - c.expected_commission_usd
      )
      / NULLIF(c.expected_commission_usd, 0),
      4
    )::NUMBER(9, 4) AS commission_variance_pct,
    COALESCE(ABS(100
    * (
      c.stated_commission_usd - c.expected_commission_usd
    )
    / NULLIF(
      c.expected_commission_usd,
      0
    )) <= @const("_variance_tolerance_pct"), FALSE)::BOOLEAN AS is_within_tolerance,
    (
      c.gross_sales_minor
        - c.commission_minor
        - COALESCE(c.other_fees_minor, 0) = c.net_payable_minor
    )::BOOLEAN AS statement_balances
  FROM converted AS c
)

SELECT *
FROM final
