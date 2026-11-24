MODEL (
  description """
    One row per ISO week and currency with price increases, markdowns and the average size of each
  """,
  columns (
    price_week_key (type VARCHAR, nullable false, audits [not_null, unique]),
    week_start (type DATE),
    currency_code (type VARCHAR),
    price_changes (type INT),
    increases (type INT),
    markdowns (type INT),
    average_increase_pct (type NUMBER(9, 4)),
    average_markdown_pct (type NUMBER(9, 4)),
    skus_below_30d_low (type INT),
  ),
);

WITH price_history AS (
  SELECT *
  FROM __ref("int_price_history")
),

changes AS (
  SELECT
    DATE_TRUNC('week', CONVERT_TIMEZONE('UTC', p.valid_from))::DATE AS week_start,
    p.currency_code,
    p.change_pct,
    p.is_markdown,
    p.sku,
    (
      p.list_amount_minor < p.lowest_price_30d_minor
    ) AS is_below_30d_low
  FROM price_history AS p
  WHERE p.previous_amount_minor IS NOT NULL
),

final AS (
  SELECT
    (
      TO_VARCHAR(c.week_start, 'YYYY-MM-DD') || ':' || c.currency_code
    )::VARCHAR AS price_week_key,
    c.week_start,
    c.currency_code,
    COUNT(*)::INT AS price_changes,
    COUNT_IF(c.change_pct > 0)::INT AS increases,
    COUNT_IF(c.is_markdown)::INT AS markdowns,
    ROUND(AVG(IFF(c.change_pct > 0, c.change_pct, NULL)), 4)::NUMBER(9, 4) AS average_increase_pct,
    ROUND(AVG(IFF(c.is_markdown, c.change_pct, NULL)), 4)::NUMBER(9, 4) AS average_markdown_pct,
    COUNT(DISTINCT IFF(c.is_below_30d_low, c.sku, NULL))::INT AS skus_below_30d_low
  FROM changes AS c
  GROUP BY
    c.week_start,
    c.currency_code
)

SELECT *
FROM final
