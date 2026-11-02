MODEL (
  description "One row per live loyalty account with its tier history as an array",
  columns (
    loyalty_account_id (type VARCHAR, nullable false, audits [not_null, unique]),
    customer_id (type VARCHAR, audits [not_null, unique]),
    tier_code (type VARCHAR),
    points_balance (type INT),
    enrolled_at (type TIMESTAMP_TZ),
    tier_change_count (type INT),
    previous_tier_code (type VARCHAR),
    tier_history (type ARRAY),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__loyalty_accounts")
),

@sql.cdc_latest(src, src.key := (loyalty_account_id))

final AS (
  SELECT
    l.loyalty_account_id,
    l.customer_id,
    UPPER(l.tier_code)::VARCHAR AS tier_code,
    GREATEST(COALESCE(l.points_balance, 0), 0)::INT AS points_balance,
    l.enrolled_at,
    ARRAY_SIZE(COALESCE(l.tier_history, ARRAY_CONSTRUCT()))::INT AS tier_change_count,
    UPPER(
      GET(GET(l.tier_history, ARRAY_SIZE(l.tier_history) - 2), 'tier')::VARCHAR
    )::VARCHAR AS previous_tier_code,
    l.tier_history::ARRAY AS tier_history,
    @sql.load_metadata(l)
  FROM cdc_latest AS l
)
SELECT *
FROM final
