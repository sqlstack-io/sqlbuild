MODEL (
  description """
    One row per currency and liability status with the outstanding gift card balance in USD
  """,
  columns (
    liability_key (type VARCHAR, nullable false, audits [not_null, unique]),
    currency_code (type VARCHAR),
    liability_status (type VARCHAR),
    card_count (type INT),
    balance_usd (type NUMBER(18, 2), audits [non_negative]),
    expiring_90d_usd (type NUMBER(18, 2)),
    dormant_card_count (type INT),
  ),
);

WITH cards AS (
  SELECT *
  FROM __ref("int_gift_card_balances")
),

final AS (
  SELECT
    (
      c.currency_code || ':' || c.liability_status
    )::VARCHAR AS liability_key,
    c.currency_code,
    c.liability_status,
    COUNT(*)::INT AS card_count,
    SUM(GREATEST(c.balance_usd, 0))::NUMBER(18, 2) AS balance_usd,
    SUM(
      IFF(c.expires_on <= DATEADD('day', 90, '@@valuation_date'::DATE), c.balance_usd, 0)
    )::NUMBER(18, 2) AS expiring_90d_usd,
    COUNT_IF(
      c.last_used_at < DATEADD('year', -1, '@@valuation_date'::TIMESTAMP_TZ)
    )::INT AS dormant_card_count
  FROM cards AS c
  GROUP BY
    c.currency_code,
    c.liability_status
)

SELECT *
FROM final
