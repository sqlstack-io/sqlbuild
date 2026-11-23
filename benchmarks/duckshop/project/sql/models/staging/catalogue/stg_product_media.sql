MODEL (
  description "One row per live product image or video with its pixel dimensions",
  columns (
    media_id (type VARCHAR, nullable false, audits [not_null, unique]),
    product_id (type VARCHAR, audits [not_null]),
    media_url (type VARCHAR),
    media_type (type VARCHAR, audits [accepted_values (values ["image", "video", "model3d"])]),
    position (type INT),
    width_px (type INT),
    height_px (type INT),
    aspect_ratio (type NUMBER(8, 4)),
    file_extension (type VARCHAR),
    dimensions (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("catalogue__product_media")
),

@sql.cdc_latest(src, src.key := (media_id))

final AS (
  SELECT
    m.media_id,
    m.product_id,
    REGEXP_REPLACE(m.url, '^http://', 'https://')::VARCHAR AS media_url,
    LOWER(m.media_type)::VARCHAR AS media_type,
    m.position::INT AS position,
    m.dimensions:width::INT AS width_px,
    m.dimensions:height::INT AS height_px,
    ROUND(
      m.dimensions:width::INT / NULLIF(m.dimensions:height::INT, 0),
      4
    )::NUMBER(8, 4) AS aspect_ratio,
    LOWER(
      REGEXP_SUBSTR(m.url, '\\.([A-Za-z0-9]{2,5})(\\?|$)', 1, 1, 'e', 1)
    )::VARCHAR AS file_extension,
    m.dimensions::OBJECT AS dimensions,
    @sql.load_metadata(m)
  FROM cdc_latest AS m
)
SELECT *
FROM final
