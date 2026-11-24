MODEL (
  description """
    One row per order with captured, refunded and charged-back amounts in minor units, pivoted by
    payment method, plus gift card and store credit tender.
  """,
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    card_captured_minor (type INT),
    wallet_captured_minor (type INT),
    bank_transfer_captured_minor (type INT),
    other_captured_minor (type INT),
    captured_minor (type INT),
    refunded_minor (type INT),
    chargeback_minor (type INT),
    gift_card_tender_minor (type INT),
    net_collected_minor (type INT),
    failed_attempts (type INT),
    tender_methods (type ARRAY),
  ),
);

WITH attempts AS (
  SELECT *
  FROM __ref("int_payment_attempts")
),

refunds AS (
  SELECT *
  FROM __ref("stg_refunds")
),

chargebacks AS (
  SELECT *
  FROM __ref("stg_chargebacks")
),

gift_card_transactions AS (
  SELECT *
  FROM __ref("stg_gift_card_transactions")
),

successful AS (
  SELECT
    a.order_id,
    CASE
      WHEN a.payment_method IN ('card', 'credit_card', 'debit_card')
      THEN 'CARD'
      WHEN a.payment_method IN ('wallet', 'phone_wallet', 'browser_wallet', 'account_wallet')
      THEN 'WALLET'
      WHEN a.payment_method IN ('bank_transfer', 'sepa', 'bank_redirect')
      THEN 'BANK'
      ELSE 'OTHER'
    END AS method_group,
    a.amount_minor
  FROM attempts AS a
  WHERE a.is_successful
),

captured_by_method AS (
  SELECT
    p.order_id,
    COALESCE(p.card, 0) AS card_captured_minor,
    COALESCE(p.wallet, 0) AS wallet_captured_minor,
    COALESCE(p.bank, 0) AS bank_transfer_captured_minor,
    COALESCE(p.other, 0) AS other_captured_minor
  FROM successful PIVOT(
    SUM(amount_minor) FOR method_group IN ('CARD', 'WALLET', 'BANK', 'OTHER')
  ) AS p(order_id, card, wallet, bank, other)
),

attempt_stats AS (
  SELECT
    a.order_id,
    COUNT_IF(NOT a.is_successful) AS failed_attempts,
    ARRAY_AGG(DISTINCT a.payment_method) WITHIN GROUP (ORDER BY a.payment_method) AS tender_methods
  FROM attempts AS a
  GROUP BY
    a.order_id
),

payment_orders AS (
  SELECT
    a.payment_id,
    a.order_id
  FROM attempts AS a
),

refunded AS (
  SELECT
    po.order_id,
    SUM(r.amount_minor) AS refunded_minor
  FROM refunds AS r
  INNER JOIN payment_orders AS po
    ON r.payment_id = po.payment_id
  GROUP BY
    po.order_id
),

charged_back AS (
  SELECT
    po.order_id,
    SUM(c.amount_minor) AS chargeback_minor
  FROM chargebacks AS c
  INNER JOIN payment_orders AS po
    ON c.payment_id = po.payment_id
  WHERE c.outcome <> 'won'
  GROUP BY
    po.order_id
),

gift_card_tender AS (
  SELECT
    g.order_id,
    -SUM(g.signed_amount_minor) AS gift_card_tender_minor
  FROM gift_card_transactions AS g
  WHERE
    g.transaction_type IN ('REDEEM', 'REVERSAL')
    AND g.order_id IS NOT NULL
  GROUP BY
    g.order_id
),

final AS (
  SELECT
    s.order_id,
    COALESCE(m.card_captured_minor, 0)::INT AS card_captured_minor,
    COALESCE(m.wallet_captured_minor, 0)::INT AS wallet_captured_minor,
    COALESCE(m.bank_transfer_captured_minor, 0)::INT AS bank_transfer_captured_minor,
    COALESCE(m.other_captured_minor, 0)::INT AS other_captured_minor,
    (
      COALESCE(m.card_captured_minor, 0)
        + COALESCE(m.wallet_captured_minor, 0)
        + COALESCE(m.bank_transfer_captured_minor, 0)
        + COALESCE(m.other_captured_minor, 0)
    )::INT AS captured_minor,
    COALESCE(r.refunded_minor, 0)::INT AS refunded_minor,
    COALESCE(cb.chargeback_minor, 0)::INT AS chargeback_minor,
    COALESCE(g.gift_card_tender_minor, 0)::INT AS gift_card_tender_minor,
    (
      COALESCE(m.card_captured_minor, 0)
        + COALESCE(m.wallet_captured_minor, 0)
        + COALESCE(m.bank_transfer_captured_minor, 0)
        + COALESCE(m.other_captured_minor, 0)
        + COALESCE(g.gift_card_tender_minor, 0)
        - COALESCE(r.refunded_minor, 0)
        - COALESCE(cb.chargeback_minor, 0)
    )::INT AS net_collected_minor,
    s.failed_attempts::INT AS failed_attempts,
    s.tender_methods::ARRAY AS tender_methods
  FROM attempt_stats AS s
  LEFT JOIN captured_by_method AS m
    ON s.order_id = m.order_id
  LEFT JOIN refunded AS r
    ON s.order_id = r.order_id
  LEFT JOIN charged_back AS cb
    ON s.order_id = cb.order_id
  LEFT JOIN gift_card_tender AS g
    ON s.order_id = g.order_id
)

SELECT *
FROM final
