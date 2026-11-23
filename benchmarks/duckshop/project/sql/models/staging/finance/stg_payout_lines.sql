MODEL (
  description "One row per payment settled in a payout, plus one line for the payout-level fee",
  columns (
    payout_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    payout_id (type VARCHAR, audits [not_null]),
    payment_id (type VARCHAR),
    line_type (type VARCHAR, audits [
      accepted_values (values ["charge", "refund", "adjustment", "payout_fee"]),
    ]),
    gross_minor (type INT),
    fee_minor (type INT),
  ),
);

WITH payouts AS (
  SELECT *
  FROM __ref("stg_payouts")
),

payment_lines AS (
  SELECT
    (
      p.payout_id || ':' || l.index::VARCHAR
    ) AS payout_line_key,
    p.payout_id,
    l.value:payment_id::VARCHAR AS payment_id,
    LOWER(COALESCE(l.value:type::VARCHAR, 'charge')) AS line_type,
    l.value:gross_minor::INT AS gross_minor,
    COALESCE(l.value:fee_minor::INT, 0) AS fee_minor
  FROM payouts AS p, LATERAL FLATTEN(input => p.lines) AS l
),

fee_lines AS (
  SELECT
    (
      p.payout_id || ':fee'
    ) AS payout_line_key,
    p.payout_id,
    NULL AS payment_id,
    'payout_fee' AS line_type,
    0 AS gross_minor,
    p.fee_minor
  FROM payouts AS p
  WHERE p.fee_minor <> 0
),

final AS (
  SELECT
    pl.payout_line_key::VARCHAR AS payout_line_key,
    pl.payout_id,
    pl.payment_id::VARCHAR AS payment_id,
    pl.line_type::VARCHAR AS line_type,
    pl.gross_minor::INT AS gross_minor,
    pl.fee_minor::INT AS fee_minor
  FROM payment_lines AS pl
  UNION ALL
  SELECT
    fl.payout_line_key::VARCHAR AS payout_line_key,
    fl.payout_id,
    fl.payment_id::VARCHAR AS payment_id,
    fl.line_type::VARCHAR AS line_type,
    fl.gross_minor::INT AS gross_minor,
    fl.fee_minor::INT AS fee_minor
  FROM fee_lines AS fl
)

SELECT *
FROM final
