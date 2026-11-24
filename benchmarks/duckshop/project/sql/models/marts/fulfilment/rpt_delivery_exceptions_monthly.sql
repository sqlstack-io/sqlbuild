MODEL (
  description """
    One row per month, carrier and responsibility with delivery exceptions, compensation paid and
    the most frequent exception code (the first in alphabetical order on a tie)
  """,
  columns (
    exception_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    exception_month (type DATE),
    carrier_code (type VARCHAR),
    responsibility (type VARCHAR),
    exceptions (type INT),
    shipments_affected (type INT),
    median_hours_to_resolve (type NUMBER(10, 2)),
    compensation_usd (type NUMBER(18, 2)),
    top_exception_code (type VARCHAR),
  ),
);

WITH exceptions AS (
  SELECT *
  FROM __ref("int_delivery_exceptions_classified")
),

dated_codes AS (
  SELECT
    DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', e.occurred_at))::DATE AS exception_month,
    e.carrier_code,
    e.responsibility,
    e.exception_code
  FROM exceptions AS e
  WHERE e.exception_code IS NOT NULL
),

code_counts AS (
  SELECT
    d.exception_month,
    d.carrier_code,
    d.responsibility,
    d.exception_code,
    COUNT(*) AS code_exceptions
  FROM dated_codes AS d
  GROUP BY
    d.exception_month,
    d.carrier_code,
    d.responsibility,
    d.exception_code
),

top_codes AS (
  SELECT
    cc.exception_month,
    cc.carrier_code,
    cc.responsibility,
    cc.exception_code
  FROM code_counts AS cc
  -- MODE breaks ties arbitrarily; take the first code in alphabetical order instead.
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY cc.exception_month, cc.carrier_code, cc.responsibility
      ORDER BY cc.code_exceptions DESC, cc.exception_code
    ) = 1
),

counts AS (
  SELECT
    (
      TO_VARCHAR(
        DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', e.occurred_at)),
        'YYYY-MM'
      )
      || ':' || e.carrier_code || ':' || e.responsibility
    )::VARCHAR AS exception_month_key,
    DATE_TRUNC(
      'month',
      CONVERT_TIMEZONE('UTC', e.occurred_at)
    )::DATE AS exception_month,
    e.carrier_code,
    e.responsibility,
    COUNT(*)::INT AS exceptions,
    COUNT(DISTINCT e.shipment_id)::INT AS shipments_affected,
    ROUND(MEDIAN(e.hours_to_resolve), 2)::NUMBER(10, 2) AS median_hours_to_resolve,
    SUM(e.compensation_usd)::NUMBER(18, 2) AS compensation_usd
  FROM exceptions AS e
  GROUP BY
    DATE_TRUNC(
      'month',
      CONVERT_TIMEZONE('UTC', e.occurred_at)
    ),
    e.carrier_code,
    e.responsibility
),

final AS (
  SELECT
    c.exception_month_key,
    c.exception_month,
    c.carrier_code,
    c.responsibility,
    c.exceptions,
    c.shipments_affected,
    c.median_hours_to_resolve,
    c.compensation_usd,
    t.exception_code::VARCHAR AS top_exception_code
  FROM counts AS c
  LEFT JOIN top_codes AS t
    ON c.exception_month = t.exception_month
    AND c.carrier_code = t.carrier_code
    AND c.responsibility = t.responsibility
)

SELECT *
FROM final
