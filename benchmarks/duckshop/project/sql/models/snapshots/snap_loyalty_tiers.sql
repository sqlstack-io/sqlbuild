MODEL (
  description """
    Version history of each loyalty account's tier, recording a new version when the tier changes
  """,
  materialized snapshot,
  unique_key [loyalty_account_id],
  snapshot_strategy check,
  check_columns [tier_code],
  columns (
    loyalty_account_id (type VARCHAR, nullable false, audits [not_null]),
    customer_id (type VARCHAR),
    tier_code (type VARCHAR),
  ),
);

WITH accounts AS (
  SELECT *
  FROM __ref("stg_loyalty_accounts")
),

final AS (
  SELECT
    a.loyalty_account_id,
    a.customer_id,
    a.tier_code
  FROM accounts AS a
)

SELECT *
FROM final
