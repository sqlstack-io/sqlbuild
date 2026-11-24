MODEL (
  description """
    One row per partner payload message with its item lines totalled and filtered through lambdas,
    including a lambda parameter named like a column of the model
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR),
    item_total (type NUMBER(18, 2)),
    priced_items (type ARRAY),
    item_skus (type ARRAY),
    priced_item_count (type INT),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.payload
  FROM __ref("stg_edge_partner_payloads") AS s
),

final AS (
  SELECT
    p.message_id,
    p.payload:sku::VARCHAR AS sku,
    REDUCE(
      p.payload:items,
      0::NUMBER(18, 2),
      (acc NUMBER(18, 2), i VARIANT) -> acc
        + COALESCE(i:qty::NUMBER(18, 2) * i:price::NUMBER(18, 2), 0)
    )::NUMBER(18, 2) AS item_total,
    FILTER(p.payload:items, sku -> sku:price IS NOT NULL)::ARRAY AS priced_items,
    TRANSFORM(
      p.payload:items,
      i -> i:sku::VARCHAR
    )::ARRAY AS item_skus,
    ARRAY_SIZE(FILTER(p.payload:items, sku -> sku:price IS NOT NULL))::INT AS priced_item_count
  FROM payloads AS p
)

SELECT *
FROM final
