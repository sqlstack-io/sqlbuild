MODEL (
  description """
    One row per customer with their current default shipping address, falling back to the latest
    address
  """,
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    address_id (type VARCHAR, audits [not_null]),
    city (type VARCHAR),
    postcode_canonical (type VARCHAR),
    country_code (type VARCHAR),
    region_code (type VARCHAR),
    latitude (type NUMBER(9, 6)),
    longitude (type NUMBER(9, 6)),
    is_explicit_default (type BOOLEAN),
    address_count (type INT),
    distinct_countries (type INT),
  ),
);

WITH addresses AS (
  SELECT *
  FROM __ref("stg_customer_addresses")
),

countries AS (
  SELECT *
  FROM __seed("countries")
),

ranked AS (
  SELECT
    a.customer_id,
    a.address_id,
    a.city,
    a.postcode_canonical,
    a.country_code,
    a.latitude,
    a.longitude,
    a.is_default,
    COUNT(*) OVER (PARTITION BY a.customer_id) AS address_count,
    COUNT(DISTINCT a.country_code) OVER (PARTITION BY a.customer_id) AS distinct_countries
  FROM addresses AS a
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY a.customer_id
      ORDER BY a.is_default DESC, a.valid_from DESC, a.address_id DESC
    ) = 1
),

final AS (
  SELECT
    r.customer_id,
    r.address_id,
    r.city,
    r.postcode_canonical,
    r.country_code,
    c.region_code::VARCHAR AS region_code,
    r.latitude,
    r.longitude,
    r.is_default AS is_explicit_default,
    r.address_count::INT AS address_count,
    r.distinct_countries::INT AS distinct_countries
  FROM ranked AS r
  LEFT JOIN countries AS c
    ON r.country_code = c.country_code
)

SELECT *
FROM final
