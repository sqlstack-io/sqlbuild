MODEL (
  description """
    One row per warehouse and month with count accuracy, out-of-tolerance counts and recount rate
  """,
  columns (
    count_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR),
    count_month (type DATE),
    counts (type INT),
    blind_counts (type INT),
    accurate_counts (type INT),
    accuracy_rate (type NUMBER(9, 4)),
    out_of_tolerance (type INT),
    net_variance_qty (type INT),
    recount_rate (type NUMBER(9, 4)),
  ),
);

WITH counts AS (
  SELECT *
  FROM __ref("int_stock_count_variances")
),

final AS (
  SELECT
    (
      c.warehouse_code || ':' || TO_VARCHAR(DATE_TRUNC('month', c.counted_at_local), 'YYYY-MM')
    )::VARCHAR AS count_month_key,
    c.warehouse_code,
    DATE_TRUNC('month', c.counted_at_local)::DATE AS count_month,
    COUNT(*)::INT AS counts,
    COUNT_IF(c.is_blind_count)::INT AS blind_counts,
    COUNT_IF(c.variance_qty = 0)::INT AS accurate_counts,
    ROUND(AVG(IFF(c.variance_qty = 0, 1, 0)), 4)::NUMBER(9, 4) AS accuracy_rate,
    COUNT_IF(c.is_out_of_tolerance)::INT AS out_of_tolerance,
    SUM(c.variance_qty)::INT AS net_variance_qty,
    ROUND(AVG(IFF(c.recount_sequence > 1, 1, 0)), 4)::NUMBER(9, 4) AS recount_rate
  FROM counts AS c
  GROUP BY
    c.warehouse_code,
    DATE_TRUNC('month', c.counted_at_local)
)

SELECT *
FROM final
