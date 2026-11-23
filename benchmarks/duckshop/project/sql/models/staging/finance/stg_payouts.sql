MODEL (
  description "One row per live payment provider payout",
  columns (
    payout_id (type VARCHAR, nullable false, audits [not_null, unique]),
    provider_code (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    amount_minor (type INT),
    fee_minor (type INT),
    net_minor (type INT),
    paid_out_on (type DATE),
    lines (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("finance__payouts")
),

@sql.cdc_latest(src, src.key := (payout_id))

final AS (
  SELECT
    p.payout_id,
    UPPER(p.provider_code)::VARCHAR AS provider_code,
    UPPER(p.currency_code)::VARCHAR AS currency_code,
    p.amount_minor::INT AS amount_minor,
    COALESCE(p.fee_minor, 0)::INT AS fee_minor,
    (
      p.amount_minor - COALESCE(p.fee_minor, 0)
    )::INT AS net_minor,
    p.paid_out_on,
    COALESCE(p.lines, ARRAY_CONSTRUCT())::ARRAY AS lines,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
