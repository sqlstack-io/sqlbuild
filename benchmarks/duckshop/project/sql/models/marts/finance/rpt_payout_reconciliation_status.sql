MODEL (
  description """
    One row per provider and payout month with payouts by reconciliation status and the unreconciled
    amount
  """,
  columns (
    payout_status_key (type VARCHAR, nullable false, audits [not_null, unique]),
    provider_code (type VARCHAR),
    payout_month (type DATE),
    payouts (type INT),
    reconciled (type INT),
    awaiting_report (type INT),
    mismatched (type INT),
    orphan_lines (type INT),
    unreconciled_net_minor (type INT),
    reconciled_share (type NUMBER(9, 4)),
  ),
);

WITH payouts AS (
  SELECT *
  FROM __ref("int_payout_reconciliation")
),

final AS (
  SELECT
    (
      p.provider_code || ':' || TO_VARCHAR(DATE_TRUNC('month', p.paid_out_on), 'YYYY-MM')
    )::VARCHAR AS payout_status_key,
    p.provider_code,
    DATE_TRUNC('month', p.paid_out_on)::DATE AS payout_month,
    COUNT(*)::INT AS payouts,
    COUNT_IF(p.reconciliation_status = 'reconciled')::INT AS reconciled,
    COUNT_IF(p.reconciliation_status = 'awaiting_report')::INT AS awaiting_report,
    COUNT_IF(p.reconciliation_status IN ('line_mismatch', 'report_mismatch'))::INT AS mismatched,
    SUM(p.orphan_line_count)::INT AS orphan_lines,
    SUM(
      IFF(p.reconciliation_status <> 'reconciled', p.payout_net_minor, 0)
    )::INT AS unreconciled_net_minor,
    ROUND(
      AVG(IFF(p.reconciliation_status = 'reconciled', 1, 0)),
      4
    )::NUMBER(9, 4) AS reconciled_share
  FROM payouts AS p
  GROUP BY
    p.provider_code,
    DATE_TRUNC('month', p.paid_out_on)
)

SELECT *
FROM final
