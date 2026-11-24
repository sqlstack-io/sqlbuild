MODEL (
  description """
    Version history of list prices rebuilt from every row of the price list change log, so price
    history is recoverable even for versions that staging has already deduplicated away.
  """,
  materialized snapshot,
  unique_key [price_list_id],
  snapshot_strategy timestamp,
  updated_at changed_at,
  observed_at loaded_at,
  historical_input changes,
  columns (
    price_list_id (type VARCHAR, nullable false, audits [not_null]),
    sku (type VARCHAR),
    currency_code (type VARCHAR),
    list_amount_minor (type INT),
    change_op (type VARCHAR),
    changed_at (type TIMESTAMP_NTZ, nullable false),
    loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH price_list_log AS (
  SELECT *
  FROM __source("catalogue__price_lists")
),

final AS (
  SELECT
    p.price_list_id,
    UPPER(TRIM(p.sku))::VARCHAR AS sku,
    UPPER(p.currency_code)::VARCHAR AS currency_code,
    p.list_amount_minor::INT AS list_amount_minor,
    p._op::VARCHAR AS change_op,
    TO_TIMESTAMP_NTZ(p._loaded_at) AS changed_at,
    TO_TIMESTAMP_NTZ(p._loaded_at) AS loaded_at
  FROM price_list_log AS p
  QUALIFY ROW_NUMBER() OVER (PARTITION BY p.price_list_id, p._loaded_at ORDER BY p._seq DESC) = 1
)

SELECT *
FROM final
