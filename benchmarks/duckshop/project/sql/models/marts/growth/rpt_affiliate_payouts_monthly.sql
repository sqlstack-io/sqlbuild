MODEL (
  description """
    One row per affiliate network and month with conversions, payable and reversed commission in USD
  """,
  columns (
    affiliate_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    affiliate_network (type VARCHAR),
    conversion_month (type DATE),
    conversions (type INT),
    last_click_conversions (type INT),
    payable_commission_usd (type NUMBER(18, 2), audits [non_negative]),
    reversed_commission_usd (type NUMBER(18, 2)),
    effective_commission_rate (type NUMBER(9, 6)),
  ),
);

WITH commissions AS (
  SELECT *
  FROM __ref("int_affiliate_commissions_usd")
),

final AS (
  SELECT
    (
      c.affiliate_network || ':'
      || TO_VARCHAR(
        DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', c.converted_at)),
        'YYYY-MM'
      )
    )::VARCHAR AS affiliate_month_key,
    c.affiliate_network,
    DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', c.converted_at))::DATE AS conversion_month,
    COUNT(*)::INT AS conversions,
    COUNT_IF(c.is_last_click)::INT AS last_click_conversions,
    SUM(c.payable_commission_usd)::NUMBER(18, 2) AS payable_commission_usd,
    SUM(
      IFF(c.is_last_click AND c.is_reversible, c.commission_usd, 0)
    )::NUMBER(18, 2) AS reversed_commission_usd,
    ROUND(
      SUM(c.payable_commission_usd) / NULLIF(SUM(IFF(c.is_last_click, c.order_amount_usd, 0)), 0),
      6
    )::NUMBER(9, 6) AS effective_commission_rate
  FROM commissions AS c
  GROUP BY
    c.affiliate_network,
    DATE_TRUNC(
      'month',
      CONVERT_TIMEZONE('UTC', c.converted_at)
    )
)

SELECT *
FROM final
