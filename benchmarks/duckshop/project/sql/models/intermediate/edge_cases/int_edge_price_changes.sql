MODEL (
  description """
    One row per ledger price change of a SKU with the price before it, skipping missing prices, the
    first and running last price under the default window frame, and its rank among the SKU's
    changes with the latest first, where a change without a time sorts first
  """,
  tags [edge_case],
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR),
    unit_price_minor (type NUMBER(38, 0)),
    previous_price (type NUMBER(38, 0)),
    previous_known_price (type NUMBER(38, 0)),
    first_price (type NUMBER(38, 0)),
    running_last_price (type NUMBER(38, 0)),
    partition_last_price (type NUMBER(38, 0)),
    recency_rank (type INT),
    price_rank (type INT),
  ),
);

WITH changes AS (
  SELECT
    e.entry_id,
    e.sku,
    e.unit_price_minor,
    e.changed_at
  FROM __ref("stg_edge_ledger_entries") AS e
  WHERE e.sku IS NOT NULL
),

final AS (
  SELECT
    c.entry_id,
    c.sku,
    c.unit_price_minor,
    LAG(c.unit_price_minor) OVER (
      PARTITION BY c.sku
      ORDER BY c.changed_at, c.entry_id
    )::NUMBER(38, 0) AS previous_price,
    LAG(c.unit_price_minor) IGNORE NULLS OVER (
      PARTITION BY c.sku
      ORDER BY c.changed_at, c.entry_id
    )::NUMBER(38, 0) AS previous_known_price,
    FIRST_VALUE(c.unit_price_minor) OVER (
      PARTITION BY c.sku
      ORDER BY c.changed_at, c.entry_id
    )::NUMBER(38, 0) AS first_price,
    LAST_VALUE(c.unit_price_minor) OVER (
      PARTITION BY c.sku
      ORDER BY c.changed_at, c.entry_id
    )::NUMBER(38, 0) AS running_last_price,
    LAST_VALUE(c.unit_price_minor) OVER (
      PARTITION BY c.sku
      ORDER BY c.changed_at, c.entry_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
    )::NUMBER(38, 0) AS partition_last_price,
    ROW_NUMBER() OVER (
      PARTITION BY c.sku
      ORDER BY c.changed_at DESC, c.entry_id DESC
    )::INT AS recency_rank,
    DENSE_RANK() OVER (PARTITION BY c.sku ORDER BY c.unit_price_minor DESC)::INT AS price_rank
  FROM changes AS c
)

SELECT *
FROM final
