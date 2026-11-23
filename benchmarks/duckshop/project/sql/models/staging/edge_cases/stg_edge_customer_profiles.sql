MODEL (
  description """
    One row per customer from the most recently updated profile snapshot, where a snapshot without
    an update time sorts first, with display names and addresses joined from parts that may be
    missing, and names in many scripts measured and re-cased
  """,
  tags [edge_case],
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    profile_id (type VARCHAR),
    profile_updated_at (type TIMESTAMP_NTZ),
    loyalty_tier (type VARCHAR),
    partner_order_number (type NUMBER(38, 0)),
    partner_order_number_float (type FLOAT),
    is_long_order_number (type BOOLEAN),
    full_name (type VARCHAR),
    previous_full_name (type VARCHAR),
    display_name (type VARCHAR),
    display_name_concat (type VARCHAR),
    display_name_skip_nulls (type VARCHAR),
    postal_address (type VARCHAR),
    name_title_case (type VARCHAR),
    name_upper (type VARCHAR),
    name_length (type INT),
    name_initial (type VARCHAR),
    first_name_padded (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__customer_profiles")
),

latest AS (
  SELECT *
  FROM src AS s
  WHERE s.customer_id IS NOT NULL
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY s.customer_id
      ORDER BY s.profile_updated_at DESC, s.profile_id DESC
    ) = 1
),

final AS (
  SELECT
    l.customer_id,
    l.profile_id,
    l.profile_updated_at,
    l.loyalty_tier,
    l.partner_order_number,
    l.partner_order_number::FLOAT AS partner_order_number_float,
    l.partner_order_number BETWEEN 9007199254740992::FLOAT AND 1e19::FLOAT AS is_long_order_number,
    l.full_name,
    l.previous_full_name,
    (
      l.first_name || ' ' || l.middle_name || ' ' || l.last_name
    )::VARCHAR AS display_name,
    CONCAT(l.first_name, ' ', l.last_name)::VARCHAR AS display_name_concat,
    CONCAT_WS(' ', l.first_name, l.middle_name, l.last_name)::VARCHAR AS display_name_skip_nulls,
    (
      l.address_line1 || ', ' || l.address_line2
    )::VARCHAR AS postal_address,
    INITCAP(l.full_name)::VARCHAR AS name_title_case,
    UPPER(l.full_name)::VARCHAR AS name_upper,
    LENGTH(l.full_name)::INT AS name_length,
    LEFT(l.full_name, 1)::VARCHAR AS name_initial,
    LPAD(l.first_name, 8, '*')::VARCHAR AS first_name_padded
  FROM latest AS l
)

SELECT *
FROM final
