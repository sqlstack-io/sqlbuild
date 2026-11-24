MODEL (
  description "One row per live supplier with payment terms and its primary contact",
  columns (
    supplier_code (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_name (type VARCHAR),
    country_code (type VARCHAR),
    payment_terms_days (type INT),
    early_payment_discount_pct (type NUMBER(5, 2)),
    primary_contact_email (type VARCHAR),
    contact_count (type INT),
    payment_terms (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__suppliers")
),

@sql.cdc_latest(src, src.key := (supplier_code))

contacts AS (
  SELECT
    s.supplier_code,
    c.value:email::VARCHAR AS contact_email,
    COALESCE(c.value:is_primary::BOOLEAN, FALSE) AS is_primary,
    c.index AS contact_index
  FROM cdc_latest AS s, LATERAL FLATTEN(input => s.contacts, outer => TRUE) AS c
), primary_contacts AS (
  SELECT
    c.supplier_code,
    c.contact_email,
    COUNT(c.contact_email) OVER (PARTITION BY c.supplier_code) AS contact_count
  FROM contacts AS c
  QUALIFY RANK() OVER (PARTITION BY c.supplier_code ORDER BY c.is_primary DESC, c.contact_index) = 1
), final AS (
  SELECT
    UPPER(s.supplier_code)::VARCHAR AS supplier_code,
    s.supplier_name,
    UPPER(s.country_code)::VARCHAR AS country_code,
    COALESCE(s.payment_terms:net_days::INT, TRY_TO_NUMBER(REGEXP_SUBSTR(
      s.payment_terms:label::VARCHAR,
      'NET\\s*([0-9]+)',
      1,
      1,
      'ie',
      1
    )))::INT AS payment_terms_days,
    s.payment_terms:early_discount_pct::NUMBER(5, 2) AS early_payment_discount_pct,
    @sql.normalise_email(pc.contact_email) AS primary_contact_email,
    pc.contact_count::INT AS contact_count,
    s.payment_terms::OBJECT AS payment_terms,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
  LEFT JOIN primary_contacts AS pc
    ON s.supplier_code = pc.supplier_code
)
SELECT *
FROM final
