MODEL (
  description """
    Version history of partner listings by change-log load time; listings removed at source are
    closed
  """,
  materialized snapshot,
  unique_key [listing_id],
  snapshot_strategy timestamp,
  updated_at source_loaded_at,
  invalidate_hard_deletes true,
  columns (
    listing_id (type VARCHAR, nullable false, audits [not_null]),
    partner_code (type VARCHAR),
    sku (type VARCHAR),
    listing_status (type VARCHAR),
    commission_pct (type NUMBER(5, 2)),
    fulfilled_by_partner (type BOOLEAN),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH listings AS (
  SELECT *
  FROM __ref("stg_partner_listings")
),

final AS (
  SELECT
    l.listing_id,
    l.partner_code,
    l.sku,
    l.listing_status,
    l.commission_pct,
    l.fulfilled_by_partner,
    l.source_loaded_at
  FROM listings AS l
)

SELECT *
FROM final
