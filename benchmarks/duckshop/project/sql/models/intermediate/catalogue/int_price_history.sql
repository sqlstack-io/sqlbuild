MODEL (
  description """
    One row per list price period of a SKU in a currency, with the previous price, the size of the
    change, and whether it was a markdown, with periods closed by the next price where the source
    left them open.
  """,
  columns (
    price_list_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR),
    list_amount_minor (type INT),
    valid_from (type TIMESTAMP_TZ),
    effective_valid_to (type TIMESTAMP_TZ),
    previous_amount_minor (type INT),
    change_pct (type NUMBER(9, 4)),
    is_markdown (type BOOLEAN),
    price_version (type INT),
    days_at_price (type INT),
    lowest_price_30d_minor (type INT),
  ),
);

WITH price_lists AS (
  SELECT *
  FROM __ref("stg_price_lists")
),

ordered AS (
  SELECT
    p.price_list_id,
    p.sku,
    p.currency_code,
    p.list_amount_minor,
    p.valid_from,
    COALESCE(
      p.valid_to,
      LEAD(p.valid_from) OVER (
        PARTITION BY p.sku, p.currency_code
        ORDER BY p.valid_from, p.price_list_id
      )
    ) AS effective_valid_to,
    LAG(p.list_amount_minor) OVER (
      PARTITION BY p.sku, p.currency_code
      ORDER BY p.valid_from, p.price_list_id
    ) AS previous_amount_minor,
    ROW_NUMBER() OVER (
      PARTITION BY p.sku, p.currency_code
      ORDER BY p.valid_from, p.price_list_id
    ) AS price_version,
    MIN(p.list_amount_minor) OVER (
      PARTITION BY p.sku, p.currency_code
      ORDER BY p.valid_from
      RANGE BETWEEN INTERVAL '30 days' PRECEDING AND CURRENT ROW
    ) AS lowest_price_30d_minor
  FROM price_lists AS p
),

final AS (
  SELECT
    o.price_list_id,
    o.sku,
    o.currency_code,
    o.list_amount_minor,
    o.valid_from,
    o.effective_valid_to,
    o.previous_amount_minor::INT AS previous_amount_minor,
    ROUND(
      100
      * (
        o.list_amount_minor - o.previous_amount_minor
      )
      / NULLIF(o.previous_amount_minor, 0),
      4
    )::NUMBER(9, 4) AS change_pct,
    COALESCE(o.list_amount_minor < o.previous_amount_minor, FALSE) AS is_markdown,
    o.price_version::INT AS price_version,
    DATEDIFF('day', o.valid_from, o.effective_valid_to)::INT AS days_at_price,
    o.lowest_price_30d_minor::INT AS lowest_price_30d_minor
  FROM ordered AS o
)

SELECT *
FROM final
