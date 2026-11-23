MODEL (
  description "One row per live list price of a SKU in one currency over a validity window",
  columns (
    price_list_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR, audits [not_null]),
    list_amount_minor (type INT, audits [not_null]),
    valid_from (type TIMESTAMP_TZ, audits [not_null]),
    valid_to (type TIMESTAMP_TZ),
    is_open_ended (type BOOLEAN),
    min_quantity (type INT),
    channel_restrictions (type ARRAY),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__price_lists")
),

@sql.cdc_latest(src, src.key := (price_list_id))

final AS (
  SELECT
    p.price_list_id,
    @sql.normalise_sku(p.sku) AS sku,
    UPPER(p.currency_code)::VARCHAR AS currency_code,
    p.list_amount_minor::INT AS list_amount_minor,
    p.valid_from,
    NULLIF(p.valid_to, '9999-12-31 00:00:00 +0000'::TIMESTAMP_TZ) AS valid_to,
    (
      p.valid_to IS NULL
      OR YEAR(p.valid_to) = 9999
    ) AS is_open_ended,
    COALESCE(p.conditions:min_qty::INT, 1)::INT AS min_quantity,
    COALESCE(p.conditions:channels, ARRAY_CONSTRUCT())::ARRAY AS channel_restrictions,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
