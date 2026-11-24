MODEL (
  description "One row per live brand",
  columns (
    brand_code (type VARCHAR, nullable false, audits [not_null, unique]),
    brand_name (type VARCHAR),
    country_code (type VARCHAR),
    is_private_label (type BOOLEAN),
    sustainability_rating (type VARCHAR),
    founded_year (type INT),
    brand_metadata (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__brands")
),

@sql.cdc_latest(src, src.key := (brand_code))

final AS (
  SELECT
    UPPER(b.brand_code)::VARCHAR AS brand_code,
    TRIM(b.brand_name)::VARCHAR AS brand_name,
    UPPER(b.country_code)::VARCHAR AS country_code,
    COALESCE(b.is_private_label, FALSE) AS is_private_label,
    UPPER(@sql.json_text(b.metadata:sustainability.rating))::VARCHAR AS sustainability_rating,
    TRY_TO_NUMBER(REGEXP_SUBSTR(
      b.metadata:founded::VARCHAR,
      '(19|20)[0-9]{2}'
    ))::INT AS founded_year,
    b.metadata::OBJECT AS brand_metadata,
    @sql.load_metadata(b)
  FROM cdc_latest AS b
)
SELECT *
FROM final
