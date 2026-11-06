TEST (name "stg_edge_partner_listings__reads_quoted_camel_case_columns_by_exact_name");

WITH __source__edge__partner_listings AS (
  SELECT
    v."listingId",
    v."unitPriceMinor"::NUMBER(38, 0) AS "unitPriceMinor",
    v."listingStatus",
    '2026-03-01 10:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('LS-MALLARD-01', 1299, 'LIVE'),
    ('LS-TEAL-02', 899, 'Out_Of_Stock'),
    ('LS-EIDER-03', 5, NULL) AS v("listingId", "unitPriceMinor", "listingStatus")
),

__expected__stg_edge_partner_listings AS (
  SELECT
    'LS-MALLARD-01' AS listing_id,
    1299::NUMBER(38, 0) AS unit_price_minor,
    'live' AS listing_status
  UNION ALL
  SELECT
    'LS-TEAL-02' AS listing_id,
    899::NUMBER(38, 0) AS unit_price_minor,
    'out_of_stock' AS listing_status
  UNION ALL
  SELECT
    'LS-EIDER-03' AS listing_id,
    5::NUMBER(38, 0) AS unit_price_minor,
    CAST(NULL AS VARCHAR) AS listing_status
)
