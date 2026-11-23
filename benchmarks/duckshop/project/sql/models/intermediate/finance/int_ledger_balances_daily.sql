MODEL (
  description """
    One row per ledger account, currency and calendar day from the account's first posting to the
    valuation date, with the day's debits and credits, the closing balance in major units, and the
    month-to-date movement. Days without postings carry the previous balance forward.
  """,
  columns (
    balance_key (type VARCHAR, nullable false, audits [not_null, unique]),
    account_code (type VARCHAR, audits [not_null]),
    account_class (type VARCHAR),
    currency_code (type VARCHAR),
    calendar_date (type DATE, audits [not_null]),
    debit_minor (type INT),
    credit_minor (type INT),
    posting_count (type INT),
    closing_balance_minor (type INT),
    closing_balance (type NUMBER(38, 6)),
    month_to_date_movement_minor (type INT),
    is_month_end (type BOOLEAN),
    is_normal_side (type BOOLEAN),
  ),
);

WITH entries AS (
  SELECT *
  FROM __ref("stg_ledger_entries")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

@sql.date_spine(@const("history_start_date"), 730)

posting_days AS (
  SELECT
    e.account_code,
    e.account_class,
    e.currency_code,
    CONVERT_TIMEZONE('UTC', e.posted_at)::DATE AS posted_date,
    e.signed_amount_minor
  FROM entries AS e
), daily_postings AS (
  SELECT
    p.account_code,
    p.currency_code,
    p.posted_date,
    SUM(IFF(p.signed_amount_minor > 0, p.signed_amount_minor, 0)) AS debit_minor,
    SUM(IFF(p.signed_amount_minor < 0, -p.signed_amount_minor, 0)) AS credit_minor,
    SUM(p.signed_amount_minor) AS net_minor,
    COUNT(*) AS posting_count
  FROM posting_days AS p
  GROUP BY
    p.account_code,
    p.currency_code,
    p.posted_date
), accounts AS (
  SELECT
    p.account_code,
    p.currency_code,
    MAX(p.account_class) AS account_class,
    MIN(p.posted_date) AS first_posted_date
  FROM posting_days AS p
  GROUP BY
    p.account_code,
    p.currency_code
), account_days AS (
  SELECT
    a.account_code,
    a.currency_code,
    a.account_class,
    d.calendar_date
  FROM accounts AS a
  INNER JOIN date_spine AS d
    ON d.calendar_date >= a.first_posted_date
  WHERE d.calendar_date <= '@@valuation_date'::DATE
), filled AS (
  SELECT
    a.account_code,
    a.currency_code,
    a.account_class,
    a.calendar_date,
    COALESCE(p.debit_minor, 0) AS debit_minor,
    COALESCE(p.credit_minor, 0) AS credit_minor,
    COALESCE(p.net_minor, 0) AS net_minor,
    COALESCE(p.posting_count, 0) AS posting_count
  FROM account_days AS a
  LEFT JOIN daily_postings AS p
    ON a.account_code = p.account_code
    AND a.currency_code = p.currency_code
    AND a.calendar_date = p.posted_date
), balances AS (
  SELECT
    f.account_code,
    f.currency_code,
    f.account_class,
    f.calendar_date,
    f.debit_minor,
    f.credit_minor,
    f.posting_count,
    SUM(f.net_minor) OVER (
      PARTITION BY f.account_code, f.currency_code
      ORDER BY f.calendar_date
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS closing_balance_minor,
    SUM(f.net_minor) OVER (
      PARTITION BY f.account_code, f.currency_code, DATE_TRUNC('month', f.calendar_date)
      ORDER BY f.calendar_date
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS month_to_date_movement_minor
  FROM filled AS f
), final AS (
  SELECT
    (
      b.account_code || ':' || b.currency_code || ':' || TO_VARCHAR(b.calendar_date, 'YYYY-MM-DD')
    )::VARCHAR AS balance_key,
    b.account_code,
    b.account_class::VARCHAR AS account_class,
    b.currency_code,
    b.calendar_date,
    b.debit_minor::INT AS debit_minor,
    b.credit_minor::INT AS credit_minor,
    b.posting_count::INT AS posting_count,
    b.closing_balance_minor::INT AS closing_balance_minor,
    @sql.money_from_minor(b.closing_balance_minor, COALESCE(c.minor_unit_exponent, 2)) AS closing_balance,
    b.month_to_date_movement_minor::INT AS month_to_date_movement_minor,
    (
      b.calendar_date = LAST_DAY(b.calendar_date)
    )::BOOLEAN AS is_month_end,
    CASE
      WHEN b.account_class IN ('asset', 'expense')
      THEN b.closing_balance_minor >= 0
      ELSE b.closing_balance_minor <= 0
    END::BOOLEAN AS is_normal_side
  FROM balances AS b
  LEFT JOIN currencies AS c
    ON b.currency_code = c.currency_code
)
SELECT *
FROM final
