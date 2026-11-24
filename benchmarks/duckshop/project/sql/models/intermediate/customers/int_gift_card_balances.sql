MODEL (
  description """
    One row per gift card with its remaining balance in major units and USD and its liability status
  """,
  columns (
    gift_card_id (type VARCHAR, nullable false, audits [not_null, unique]),
    currency_code (type VARCHAR),
    issued_at (type TIMESTAMP_TZ),
    expires_on (type DATE),
    initial_amount (type NUMBER(38, 6)),
    redeemed_amount (type NUMBER(38, 6)),
    balance_amount (type NUMBER(38, 6)),
    balance_usd (type NUMBER(18, 2)),
    transaction_count (type INT),
    last_used_at (type TIMESTAMP_TZ),
    liability_status (type VARCHAR, audits [
      accepted_values (values ["active", "fully_redeemed", "expired_unused", "expired_partial"]),
    ]),
  ),
);

WITH gift_cards AS (
  SELECT *
  FROM __ref("stg_gift_cards")
),

transactions AS (
  SELECT *
  FROM __ref("stg_gift_card_transactions")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

movement AS (
  SELECT
    t.gift_card_id,
    SUM(IFF(t.signed_amount_minor < 0, -t.signed_amount_minor, 0)) AS redeemed_minor,
    SUM(t.signed_amount_minor) AS net_movement_minor,
    COUNT(*) AS transaction_count,
    MAX(t.occurred_at) AS last_used_at
  FROM transactions AS t
  GROUP BY
    t.gift_card_id
),

balances AS (
  SELECT
    g.gift_card_id,
    g.currency_code,
    g.issued_at,
    g.expires_on,
    g.initial_amount_minor,
    COALESCE(m.redeemed_minor, 0) AS redeemed_minor,
    GREATEST(g.initial_amount_minor + COALESCE(m.net_movement_minor, 0), 0) AS balance_minor,
    COALESCE(m.transaction_count, 0) AS transaction_count,
    m.last_used_at,
    c.minor_unit_exponent,
    '@@valuation_date'::DATE AS valuation_date
  FROM gift_cards AS g
  INNER JOIN currencies AS c
    ON g.currency_code = c.currency_code
  LEFT JOIN movement AS m
    ON g.gift_card_id = m.gift_card_id
),

valued AS (
  SELECT
    b.gift_card_id,
    b.currency_code,
    b.issued_at,
    b.expires_on,
    b.initial_amount_minor,
    b.redeemed_minor,
    b.balance_minor,
    b.transaction_count,
    b.last_used_at,
    b.minor_unit_exponent,
    b.valuation_date,
    fx.rate_to_usd
  FROM balances AS b
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (b.valuation_date >= fx.rate_date)
    ON b.currency_code = fx.currency_code
),

final AS (
  SELECT
    v.gift_card_id,
    v.currency_code,
    v.issued_at,
    v.expires_on,
    @sql.money_from_minor(v.initial_amount_minor, v.minor_unit_exponent, places := 3) AS initial_amount,
    @sql.money_from_minor(v.redeemed_minor, v.minor_unit_exponent, places := 3) AS redeemed_amount,
    @sql.money_from_minor(v.balance_minor, v.minor_unit_exponent, places := 3) AS balance_amount,
    ROUND(
      @sql.money_from_minor(v.balance_minor, v.minor_unit_exponent, places := 6) * v.rate_to_usd,
      2
    )::NUMBER(18, 2) AS balance_usd,
    v.transaction_count::INT AS transaction_count,
    v.last_used_at,
    CASE
      WHEN v.balance_minor = 0
      THEN 'fully_redeemed'
      WHEN v.expires_on < v.valuation_date
      AND v.redeemed_minor = 0
      THEN 'expired_unused'
      WHEN v.expires_on < v.valuation_date
      THEN 'expired_partial'
      ELSE 'active'
    END::VARCHAR AS liability_status
  FROM valued AS v
)

SELECT *
FROM final
