MODEL (
  description """
    One row per carrier and dispatch week with on-time dispatch, delivery speed and exception rate
  """,
  columns (
    carrier_week_key (type VARCHAR, nullable false, audits [not_null, unique]),
    carrier_code (type VARCHAR),
    week_start (type DATE),
    shipments (type INT),
    on_time_dispatch_rate (type NUMBER(9, 4)),
    p50_hours_to_deliver (type NUMBER(10, 2)),
    p90_hours_to_deliver (type NUMBER(10, 2)),
    exception_rate (type NUMBER(9, 4)),
    compensation_usd (type NUMBER(18, 2)),
  ),
);

WITH shipments AS (
  SELECT *
  FROM __ref("fct_shipments")
),

final AS (
  SELECT
    (
      s.carrier_code || ':'
      || TO_VARCHAR(
        DATE_TRUNC('week', CONVERT_TIMEZONE('UTC', s.shipped_at)),
        'YYYY-MM-DD'
      )
    )::VARCHAR AS carrier_week_key,
    s.carrier_code,
    DATE_TRUNC('week', CONVERT_TIMEZONE('UTC', s.shipped_at))::DATE AS week_start,
    COUNT(*)::INT AS shipments,
    ROUND(AVG(IFF(s.is_dispatch_on_time, 1, 0)), 4)::NUMBER(9, 4) AS on_time_dispatch_rate,
    ROUND(
      PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY s.hours_to_deliver),
      2
    )::NUMBER(10, 2) AS p50_hours_to_deliver,
    ROUND(
      PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY s.hours_to_deliver),
      2
    )::NUMBER(10, 2) AS p90_hours_to_deliver,
    ROUND(AVG(IFF(s.exception_count > 0, 1, 0)), 4)::NUMBER(9, 4) AS exception_rate,
    SUM(s.compensation_usd)::NUMBER(18, 2) AS compensation_usd
  FROM shipments AS s
  WHERE s.shipped_at IS NOT NULL
  GROUP BY
    s.carrier_code,
    DATE_TRUNC('week', CONVERT_TIMEZONE('UTC', s.shipped_at))
)

SELECT *
FROM final
