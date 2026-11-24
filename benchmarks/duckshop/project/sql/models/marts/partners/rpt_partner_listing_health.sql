MODEL (
  description """
    One row per partner and listing health state with listings, units sold in 30 days and average
    commission
  """,
  columns (
    listing_health_key (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR),
    listing_health (type VARCHAR),
    listings (type INT),
    partner_fulfilled (type INT),
    units_sold_30d (type INT),
    average_commission_pct (type NUMBER(5, 2)),
    share_of_partner_listings (type NUMBER(9, 4)),
  ),
);

WITH listings AS (
  SELECT *
  FROM __ref("int_partner_listing_status")
),

grouped AS (
  SELECT
    l.partner_code,
    l.listing_health,
    COUNT(*) AS listings,
    COUNT_IF(l.fulfilled_by_partner) AS partner_fulfilled,
    SUM(l.units_sold_30d) AS units_sold_30d,
    AVG(l.commission_pct) AS average_commission_pct
  FROM listings AS l
  GROUP BY
    l.partner_code,
    l.listing_health
),

final AS (
  SELECT
    (
      g.partner_code || ':' || g.listing_health
    )::VARCHAR AS listing_health_key,
    g.partner_code,
    g.listing_health,
    g.listings::INT AS listings,
    g.partner_fulfilled::INT AS partner_fulfilled,
    g.units_sold_30d::INT AS units_sold_30d,
    ROUND(g.average_commission_pct, 2)::NUMBER(5, 2) AS average_commission_pct,
    ROUND(
      RATIO_TO_REPORT(g.listings) OVER (PARTITION BY g.partner_code),
      4
    )::NUMBER(9, 4) AS share_of_partner_listings
  FROM grouped AS g
)

SELECT *
FROM final
