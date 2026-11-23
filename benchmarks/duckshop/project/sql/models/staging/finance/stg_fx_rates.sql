MODEL (
  description "One row per currency and rate date, with the date the rate stops applying",
  columns (
    currency_code (type VARCHAR, nullable false, audits [not_null]),
    rate_date (type DATE, nullable false, audits [not_null]),
    rate_to_usd (type NUMBER(18, 8), audits [not_null]),
    usd_to_rate (type NUMBER(18, 8)),
    next_rate_date (type DATE),
  ),
  audits [
    unique_combination (columns "currency_code, rate_date"),
  ],
);

WITH fx_rates AS (
  SELECT *
  FROM __source("finance__fx_rates")
),

final AS (
  SELECT
    f.currency_code,
    f.rate_date,
    f.rate_to_usd,
    ROUND(1::NUMBER(38, 12) / f.rate_to_usd, 8)::NUMBER(18, 8) AS usd_to_rate,
    LEAD(f.rate_date) OVER (
      PARTITION BY f.currency_code
      ORDER BY f.rate_date
    )::DATE AS next_rate_date
  FROM fx_rates AS f
)

SELECT *
FROM final
