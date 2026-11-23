MODEL (
  description """
    One row per partner listing, reading the feed's quoted camelCase columns by their exact names
    and renaming them to snake case
  """,
  tags [edge_case],
  columns (
    listing_id (type VARCHAR, nullable false, audits [not_null, unique]),
    unit_price_minor (type NUMBER(38, 0)),
    listing_status (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__partner_listings")
),

final AS (
  SELECT
    s."listingId"::VARCHAR AS listing_id,
    s."unitPriceMinor"::NUMBER(38, 0) AS unit_price_minor,
    LOWER(s."listingStatus")::VARCHAR AS listing_status
  FROM src AS s
)

SELECT *
FROM final
