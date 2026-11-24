MODEL (
  description "One row per live store credit posting",
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR, audits [not_null]),
    amount_minor (type INT, audits [not_null]),
    reason_code (type VARCHAR),
    posted_at (type TIMESTAMP_TZ),
    source_order_id (type VARCHAR),
    expires_on (type DATE),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__store_credit_ledger")
),

@sql.cdc_latest(src, src.key := (entry_id))

final AS (
  SELECT
    e.entry_id,
    e.customer_id,
    UPPER(e.currency_code)::VARCHAR AS currency_code,
    e.amount_minor,
    UPPER(REGEXP_REPLACE(e.reason_code, '[^A-Za-z0-9]+', '_'))::VARCHAR AS reason_code,
    e.posted_at,
    @sql.json_text(e.context:order_id) AS source_order_id,
    TRY_TO_DATE(e.context:expires_on::VARCHAR) AS expires_on,
    @sql.load_metadata(e)
  FROM cdc_latest AS e
)
SELECT *
FROM final
