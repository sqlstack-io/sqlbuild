MODEL (
  description """
    Version history of customer accounts, one row per customer per change in the account change log.
    Accounts deleted at source are closed.
  """,
  materialized snapshot,
  unique_key [customer_id],
  snapshot_strategy timestamp,
  updated_at source_loaded_at,
  invalidate_hard_deletes true,
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null (run_scope delta_and_final)]),
    email_normalised (type VARCHAR),
    region_code (type VARCHAR),
    storefront_code (type VARCHAR),
    preferred_language (type VARCHAR),
    marketing_opt_in (type BOOLEAN),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

final AS (
  SELECT
    c.customer_id,
    c.email_normalised,
    c.region_code,
    c.storefront_code,
    c.preferred_language,
    c.marketing_opt_in,
    c.source_loaded_at
  FROM customers AS c
)

SELECT *
FROM final
