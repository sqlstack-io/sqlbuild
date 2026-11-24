MODEL (
  description """
    One row per partner listing, published under an enforced contract through a star over the
    staging model, whose columns Snowflake names in upper case
  """,
  tags [edge_case],
  contract enforced,
  columns (
    listing_id (type VARCHAR, nullable false, audits [not_null, unique]),
    unit_price_minor (type NUMBER(38, 0)),
    listing_status (type VARCHAR),
  ),
);

WITH final AS (
  SELECT s.*
  FROM __ref("stg_edge_partner_listings") AS s
)

SELECT *
FROM final
