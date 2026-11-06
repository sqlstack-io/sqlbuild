TEST (name "snap_edge_partner_listing_export__tracks_listings_with_a_known_status");

WITH __ref__rpt_edge_partner_listing_export AS (
  SELECT
    'LS-EIDER-03' AS "listingId",
    0.05::NUMBER(18, 2) AS "unitPrice",
    CAST(NULL AS BOOLEAN) AS "isLive"
  UNION ALL
  SELECT
    'LS-MALLARD-01' AS "listingId",
    12.99::NUMBER(18, 2) AS "unitPrice",
    TRUE AS "isLive"
  UNION ALL
  SELECT
    'LS-TEAL-02' AS "listingId",
    8.99::NUMBER(18, 2) AS "unitPrice",
    FALSE AS "isLive"
),

__expected__snap_edge_partner_listing_export AS (
  SELECT
    'LS-MALLARD-01' AS "listingId",
    12.99::NUMBER(18, 2) AS "unitPrice",
    TRUE AS "isLive"
  UNION ALL
  SELECT
    'LS-TEAL-02' AS "listingId",
    8.99::NUMBER(18, 2) AS "unitPrice",
    FALSE AS "isLive"
)
