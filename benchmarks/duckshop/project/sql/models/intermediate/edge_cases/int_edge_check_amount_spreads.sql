MODEL (
  description """
    One row per check group with the median of its amounts, and per group the product of an
    exact decimal rate with one less the rate
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    median_amount (type NUMBER(18, 3)),
    rate_spread (type NUMBER(38, 12)),
  ),
);

WITH rates AS (
  SELECT
    a.group_code,
    a.amount,
    a.amount / NULLIF(a.wide_key, 0) AS rate
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    r.group_code,
    MEDIAN(r.amount)::NUMBER(18, 3) AS median_amount,
    MAX(r.rate * (1 - r.rate))::NUMBER(38, 12) AS rate_spread
  FROM rates AS r
  GROUP BY
    r.group_code
)

SELECT *
FROM final
