MODEL (
  description """
    One row per provider payout reconciled against its own line items, the payments those lines
    reference, and the provider's settlement report for the payout date. Lines that reference no
    known payment are counted as orphans.
  """,
  constants (
    _tolerance_minor 5,
  ),
  columns (
    payout_id (type VARCHAR, nullable false, audits [not_null, unique]),
    provider_code (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    paid_out_on (type DATE),
    payout_net_minor (type INT),
    lines_net_minor (type INT),
    charge_line_count (type INT),
    refund_line_count (type INT),
    orphan_line_count (type INT),
    embedded_line_count (type INT),
    loaded_line_count (type INT),
    matched_payment_minor (type INT),
    report_id (type VARCHAR),
    report_net_minor (type INT),
    lines_match_payout (type BOOLEAN),
    report_matches_payout (type BOOLEAN),
    reconciliation_status (type VARCHAR),
    days_since_previous_payout (type INT),
  ),
);

WITH payouts AS (
  SELECT *
  FROM __ref("stg_payouts")
),

payout_lines AS (
  SELECT *
  FROM __ref("stg_payout_lines")
),

payments AS (
  SELECT *
  FROM __ref("stg_payments")
),

reports AS (
  SELECT *
  FROM __ref("stg_settlement_reports")
),

payout_sequence AS (
  SELECT
    p.payout_id,
    DATEDIFF(
      'day',
      LAG(p.paid_out_on) OVER (
        PARTITION BY p.provider_code, p.currency_code
        ORDER BY p.paid_out_on, p.payout_id
      ),
      p.paid_out_on
    ) AS days_since_previous_payout,
    ARRAY_SIZE(COALESCE(p.lines, ARRAY_CONSTRUCT())) AS embedded_line_count
  FROM payouts AS p
),

line_totals AS (
  SELECT
    l.payout_id,
    SUM(
      IFF(l.line_type = 'refund', -ABS(l.gross_minor), l.gross_minor) - COALESCE(l.fee_minor, 0)
    ) AS lines_net_minor,
    COUNT_IF(l.line_type = 'charge') AS charge_line_count,
    COUNT_IF(l.line_type = 'refund') AS refund_line_count,
    COUNT(*) AS loaded_line_count
  FROM payout_lines AS l
  GROUP BY
    l.payout_id
),

orphan_payment_ids AS (
  SELECT l.payment_id::VARCHAR AS payment_id
  FROM payout_lines AS l
  WHERE l.payment_id IS NOT NULL
  MINUS
  SELECT p.payment_id::VARCHAR AS payment_id
  FROM payments AS p
),

line_matches AS (
  SELECT
    l.payout_id,
    COUNT(o.payment_id) AS orphan_line_count,
    SUM(
      IFF(o.payment_id IS NULL AND l.line_type = 'charge', p.amount_minor, 0)
    ) AS matched_payment_minor
  FROM payout_lines AS l
  LEFT JOIN orphan_payment_ids AS o
    ON l.payment_id = o.payment_id
  LEFT JOIN payments AS p
    ON l.payment_id = p.payment_id
  GROUP BY
    l.payout_id
),

daily_reports AS (
  SELECT
    r.provider_code,
    r.report_date,
    r.currency_code,
    r.report_id,
    r.gross_minor - COALESCE(r.fees_minor, 0) AS report_net_minor
  FROM reports AS r
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY r.provider_code, r.report_date, r.currency_code
      ORDER BY r.line_count DESC, r.report_id DESC
    ) = 1
),

joined AS (
  SELECT
    p.payout_id,
    p.provider_code,
    p.currency_code,
    p.paid_out_on,
    p.net_minor AS payout_net_minor,
    COALESCE(t.lines_net_minor, 0) AS lines_net_minor,
    COALESCE(t.charge_line_count, 0) AS charge_line_count,
    COALESCE(t.refund_line_count, 0) AS refund_line_count,
    COALESCE(m.orphan_line_count, 0) AS orphan_line_count,
    s.embedded_line_count,
    COALESCE(t.loaded_line_count, 0) AS loaded_line_count,
    COALESCE(m.matched_payment_minor, 0) AS matched_payment_minor,
    r.report_id,
    r.report_net_minor,
    s.days_since_previous_payout
  FROM payouts AS p
  INNER JOIN payout_sequence AS s
    ON p.payout_id = s.payout_id
  LEFT JOIN line_totals AS t
    ON p.payout_id = t.payout_id
  LEFT JOIN line_matches AS m
    ON p.payout_id = m.payout_id
  LEFT JOIN daily_reports AS r
    ON p.provider_code = r.provider_code
    AND p.paid_out_on = r.report_date
    AND p.currency_code = r.currency_code
),

checked AS (
  SELECT
    j.payout_id,
    j.provider_code,
    j.currency_code,
    j.paid_out_on,
    j.payout_net_minor,
    j.lines_net_minor,
    j.charge_line_count,
    j.refund_line_count,
    j.orphan_line_count,
    j.embedded_line_count,
    j.loaded_line_count,
    j.matched_payment_minor,
    j.report_id,
    j.report_net_minor,
    j.days_since_previous_payout,
    ABS(j.payout_net_minor - j.lines_net_minor) <= @const("_tolerance_minor") AS lines_match_payout,
    COALESCE(
      ABS(j.payout_net_minor - j.report_net_minor) <= @const("_tolerance_minor"),
      FALSE
    ) AS report_matches_payout
  FROM joined AS j
),

final AS (
  SELECT
    c.payout_id,
    c.provider_code,
    c.currency_code,
    c.paid_out_on,
    c.payout_net_minor::INT AS payout_net_minor,
    c.lines_net_minor::INT AS lines_net_minor,
    c.charge_line_count::INT AS charge_line_count,
    c.refund_line_count::INT AS refund_line_count,
    c.orphan_line_count::INT AS orphan_line_count,
    c.embedded_line_count::INT AS embedded_line_count,
    c.loaded_line_count::INT AS loaded_line_count,
    c.matched_payment_minor::INT AS matched_payment_minor,
    c.report_id,
    c.report_net_minor::INT AS report_net_minor,
    c.lines_match_payout::BOOLEAN AS lines_match_payout,
    c.report_matches_payout::BOOLEAN AS report_matches_payout,
    CASE
      WHEN c.lines_match_payout
      AND c.report_matches_payout
      AND c.orphan_line_count = 0
      THEN 'reconciled'
      WHEN c.report_id IS NULL
      THEN 'awaiting_report'
      WHEN c.orphan_line_count > 0
      THEN 'orphan_lines'
      WHEN NOT c.lines_match_payout
      THEN 'line_mismatch'
      ELSE 'report_mismatch'
    END::VARCHAR AS reconciliation_status,
    c.days_since_previous_payout::INT AS days_since_previous_payout
  FROM checked AS c
)

SELECT *
FROM final
