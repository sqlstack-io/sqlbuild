MODEL (
  description "One row per live loyalty points posting",
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    loyalty_account_id (type VARCHAR, audits [not_null]),
    points_delta (type INT, audits [not_null]),
    reason_code (type VARCHAR),
    order_id (type VARCHAR),
    posted_at (type TIMESTAMP_TZ, audits [not_null]),
    campaign_ref (type VARCHAR),
    expires_on (type DATE),
    metadata (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__loyalty_ledger")
),

@sql.cdc_latest(src, src.key := (entry_id))

final AS (
  SELECT
    e.entry_id,
    e.loyalty_account_id,
    e.points_delta,
    UPPER(e.reason_code)::VARCHAR AS reason_code,
    NULLIF(e.order_id, '')::VARCHAR AS order_id,
    e.posted_at,
    @sql.json_text(e.metadata:campaign) AS campaign_ref,
    TRY_TO_DATE(e.metadata:expires_on::VARCHAR)::DATE AS expires_on,
    e.metadata::OBJECT AS metadata,
    @sql.load_metadata(e)
  FROM cdc_latest AS e
)
SELECT *
FROM final
