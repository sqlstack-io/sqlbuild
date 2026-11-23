MODEL (
  description """
    One row per order ledger line with money arithmetic at its edges: decimal division and zero
    divisors, rounding half-way values as NUMBER and FLOAT, FLOAT exchange rates cast to fixed
    scales, MOD bucketing of negative and fractional values, and 38-digit amounts
  """,
  tags [edge_case],
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    unit_amount (type NUMBER(38, 10)),
    share_of_three (type NUMBER(38, 10)),
    unit_amount_div0 (type NUMBER(38, 10)),
    unit_amount_nullifzero (type NUMBER(38, 10)),
    amount_rounded (type NUMBER(38, 2)),
    amount_rounded_whole (type NUMBER(38, 0)),
    amount_float_rounded (type FLOAT),
    converted_minor (type FLOAT),
    converted_2dp (type NUMBER(18, 2)),
    converted_6dp (type NUMBER(18, 6)),
    converted_rounded (type NUMBER(18, 2)),
    line_bucket (type NUMBER(38, 10)),
    rate_bucket (type FLOAT),
    amount_halved (type NUMBER(38, 10)),
  ),
);

WITH lines AS (
  SELECT
    e.entry_id,
    e.quantity,
    e.amount,
    e.unit_price_minor,
    e.fx_rate
  FROM __ref("stg_edge_ledger_entries") AS e
),

final AS (
  SELECT
    l.entry_id,
    (
      l.amount / NULLIF(l.quantity, 0)
    )::NUMBER(38, 10) AS unit_amount,
    (
      l.amount / 3
    )::NUMBER(38, 10) AS share_of_three,
    DIV0(l.amount, l.quantity)::NUMBER(38, 10) AS unit_amount_div0,
    (
      l.amount / NULLIFZERO(l.quantity)
    )::NUMBER(38, 10) AS unit_amount_nullifzero,
    ROUND(l.amount, 2)::NUMBER(38, 2) AS amount_rounded,
    ROUND(l.amount)::NUMBER(38, 0) AS amount_rounded_whole,
    ROUND(l.amount::FLOAT)::FLOAT AS amount_float_rounded,
    (
      l.unit_price_minor * l.fx_rate
    )::FLOAT AS converted_minor,
    (
      l.unit_price_minor * l.fx_rate
    )::NUMBER(18, 2) AS converted_2dp,
    (
      l.unit_price_minor * l.fx_rate
    )::NUMBER(18, 6) AS converted_6dp,
    ROUND(l.unit_price_minor * l.fx_rate, 2)::NUMBER(18, 2) AS converted_rounded,
    MOD(l.amount, 7)::NUMBER(38, 10) AS line_bucket,
    MOD(l.fx_rate, 0.5)::FLOAT AS rate_bucket,
    (
      l.amount / 2
    )::NUMBER(38, 10) AS amount_halved
  FROM lines AS l
)

SELECT *
FROM final
