MODEL (
  description """
    One row per month and return reason with return volume, resellable share and refunded USD
  """,
  columns (
    return_reason_key (type VARCHAR, nullable false, audits [not_null, unique]),
    return_month (type DATE),
    return_reason (type return_reason),
    returns (type INT),
    units (type INT),
    resellable_share (type NUMBER(9, 4)),
    refund_usd (type NUMBER(18, 2)),
    median_days_to_refund (type NUMBER(10, 2)),
    share_of_month (type NUMBER(9, 4)),
  ),
);

WITH reasons AS (
  SELECT *
  FROM __ref("int_return_reasons")
),

outcomes AS (
  SELECT *
  FROM __ref("int_return_outcomes")
),

monthly AS (
  SELECT
    DATE_TRUNC('month', CONVERT_TIMEZONE('UTC', r.requested_at))::DATE AS return_month,
    r.return_reason,
    COUNT(*) AS returns,
    SUM(r.qty) AS units,
    AVG(IFF(o.is_resellable, 1, 0)) AS resellable_share,
    SUM(o.refund_usd) AS refund_usd,
    MEDIAN(o.days_to_refund) AS median_days_to_refund
  FROM reasons AS r
  LEFT JOIN outcomes AS o
    ON r.return_id = o.return_id
  GROUP BY
    1,
    2
),

final AS (
  SELECT
    (
      TO_VARCHAR(m.return_month, 'YYYY-MM') || ':' || m.return_reason
    )::VARCHAR AS return_reason_key,
    m.return_month,
    m.return_reason::VARCHAR AS return_reason,
    m.returns::INT AS returns,
    m.units::INT AS units,
    ROUND(m.resellable_share, 4)::NUMBER(9, 4) AS resellable_share,
    m.refund_usd::NUMBER(18, 2) AS refund_usd,
    ROUND(m.median_days_to_refund, 2)::NUMBER(10, 2) AS median_days_to_refund,
    ROUND(
      RATIO_TO_REPORT(m.returns) OVER (PARTITION BY m.return_month),
      4
    )::NUMBER(9, 4) AS share_of_month
  FROM monthly AS m
)

SELECT *
FROM final
