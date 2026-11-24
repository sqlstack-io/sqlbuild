MODEL (
  description """
    One row per ordering customer with recency, frequency and monetary scores in quintiles and a
    named segment.
  """,
  enums (
    _segment [CHAMPION, LOYAL, PROMISING, AT_RISK, HIBERNATING, NEW],
  ),
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    last_order_date (type DATE),
    days_since_last_order (type INT),
    order_count (type INT),
    lifetime_revenue_usd (type NUMBER(18, 2)),
    average_order_usd (type NUMBER(18, 2)),
    recency_score (type INT),
    frequency_score (type INT),
    monetary_score (type INT),
    rfm_code (type VARCHAR),
    segment (type VARCHAR),
    is_vip (type BOOLEAN),
  ),
);

WITH order_sequence AS (
  SELECT *
  FROM __ref("int_customer_order_sequence")
),

as_of AS (
  SELECT MAX(o.placed_date_local) AS as_of_date
  FROM order_sequence AS o
),

per_customer AS (
  SELECT
    o.customer_id,
    MAX(o.placed_date_local) AS last_order_date,
    COUNT(*) AS order_count,
    MAX(o.lifetime_revenue_usd) AS lifetime_revenue_usd
  FROM order_sequence AS o
  GROUP BY
    o.customer_id
),

scored AS (
  SELECT
    p.customer_id,
    p.last_order_date,
    DATEDIFF('day', p.last_order_date, a.as_of_date) AS days_since_last_order,
    p.order_count,
    p.lifetime_revenue_usd,
    6
      - NTILE(5) OVER (
        ORDER BY DATEDIFF('day', p.last_order_date, a.as_of_date), p.customer_id
      ) AS recency_score,
    NTILE(5) OVER (ORDER BY p.order_count, p.customer_id) AS frequency_score,
    NTILE(5) OVER (ORDER BY p.lifetime_revenue_usd, p.customer_id) AS monetary_score
  FROM per_customer AS p
  CROSS JOIN as_of AS a
),

final AS (
  SELECT
    s.customer_id,
    s.last_order_date,
    s.days_since_last_order::INT AS days_since_last_order,
    s.order_count::INT AS order_count,
    s.lifetime_revenue_usd,
    ROUND(s.lifetime_revenue_usd / NULLIF(s.order_count, 0), 2)::NUMBER(18, 2) AS average_order_usd,
    s.recency_score::INT AS recency_score,
    s.frequency_score::INT AS frequency_score,
    s.monetary_score::INT AS monetary_score,
    (
      s.recency_score::VARCHAR || s.frequency_score::VARCHAR || s.monetary_score::VARCHAR
    )::VARCHAR AS rfm_code,
    CASE
      WHEN s.order_count = 1
      AND s.recency_score >= 4
      THEN @enum("_segment").NEW
      WHEN s.recency_score >= 4
      AND s.frequency_score >= 4
      THEN @enum("_segment").CHAMPION
      WHEN s.frequency_score >= 3
      AND s.recency_score >= 3
      THEN @enum("_segment").LOYAL
      WHEN s.recency_score >= 3
      THEN @enum("_segment").PROMISING
      WHEN s.frequency_score >= 3
      THEN @enum("_segment").AT_RISK
      ELSE @enum("_segment").HIBERNATING
    END::VARCHAR AS segment,
    (
      s.lifetime_revenue_usd >= @const("vip_ltv_threshold")
    ) AS is_vip
  FROM scored AS s
)

SELECT *
FROM final
