MODEL (
  description """
    One row per ledger account, currency and month end with the closing balance in major units and
    USD, the month's debits and credits, and whether the balance sits on its normal side.
  """,
  contract enforced,
  audit_factories [fact_table_quality],
  columns (
    ledger_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    account_code (type VARCHAR, audits [not_null]),
    account_class (type VARCHAR, audits [
      accepted_values (values ["asset", "liability", "equity", "revenue", "expense"]),
    ]),
    currency_code (type VARCHAR),
    month_end (type DATE, audits [not_null]),
    month_debit_minor (type INT),
    month_credit_minor (type INT),
    closing_balance (type NUMBER(38, 6)),
    closing_balance_usd (type NUMBER(18, 2)),
    is_normal_side (type BOOLEAN),
  ),
);

WITH balances AS (
  SELECT *
  FROM __ref("int_ledger_balances_daily")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

month_ends AS (
  SELECT
    b.account_code,
    b.account_class,
    b.currency_code,
    b.calendar_date AS month_end,
    LAST_DAY(b.calendar_date) AS month_last_day,
    b.closing_balance,
    b.is_normal_side
  FROM balances AS b
  WHERE
    b.is_month_end
    OR b.calendar_date = '@@valuation_date'::DATE
),

month_flows AS (
  SELECT
    b.account_code,
    b.currency_code,
    LAST_DAY(b.calendar_date) AS month_last_day,
    SUM(b.debit_minor) AS month_debit_minor,
    SUM(b.credit_minor) AS month_credit_minor
  FROM balances AS b
  GROUP BY
    1,
    2,
    3
),

with_rate AS (
  SELECT
    m.account_code,
    m.account_class,
    m.currency_code,
    m.month_end,
    m.month_last_day,
    m.closing_balance,
    m.is_normal_side,
    fx.rate_to_usd
  FROM month_ends AS m
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (m.month_end >= fx.rate_date)
    ON m.currency_code = fx.currency_code
),

final AS (
  SELECT
    (
      w.account_code || ':' || w.currency_code || ':' || TO_VARCHAR(w.month_end, 'YYYY-MM-DD')
    )::VARCHAR AS ledger_month_key,
    w.account_code,
    w.account_class,
    w.currency_code,
    w.month_end,
    COALESCE(f.month_debit_minor, 0)::INT AS month_debit_minor,
    COALESCE(f.month_credit_minor, 0)::INT AS month_credit_minor,
    w.closing_balance,
    ROUND(w.closing_balance * w.rate_to_usd, 2)::NUMBER(18, 2) AS closing_balance_usd,
    w.is_normal_side
  FROM with_rate AS w
  LEFT JOIN month_flows AS f
    ON w.account_code = f.account_code
    AND w.currency_code = f.currency_code
    AND w.month_last_day = f.month_last_day
)

SELECT *
FROM final
