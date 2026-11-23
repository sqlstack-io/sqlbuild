MODEL (
  description """
    One row per partner listing in the partner's own feed format, with quoted camelCase column names
    and prices in major units
  """,
  tags [edge_case],
  columns (
    "listingId" (nullable false, audits [not_null, unique]),
  ),
);

WITH listings AS (
  SELECT
    d.listing_id,
    d.unit_price_minor,
    d.listing_status
  FROM __ref("dim_edge_partner_listings") AS d
),

final AS (
  SELECT
    l.listing_id AS "listingId",
    (
      l.unit_price_minor / 100
    )::NUMBER(18, 2) AS "unitPrice",
    (
      l.listing_status = 'live'
    ) AS "isLive"
  FROM listings AS l
)

SELECT *
FROM final
