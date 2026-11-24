MODEL (
  description """
    Version history of the partner feed export for listings with a known status, keyed and compared
    on its quoted camelCase columns
  """,
  tags [edge_case],
  materialized snapshot,
  unique_key ['"listingId"'],
  snapshot_strategy check,
  check_columns ['"unitPrice"', '"isLive"'],
  columns (
    "listingId" (nullable false, audits [not_null]),
  ),
);

WITH export AS (
  SELECT *
  FROM __ref("rpt_edge_partner_listing_export")
),

final AS (
  SELECT
    e."listingId",
    e."unitPrice",
    e."isLive"
  FROM export AS e
  WHERE e."isLive" IS NOT NULL
)

SELECT *
FROM final
