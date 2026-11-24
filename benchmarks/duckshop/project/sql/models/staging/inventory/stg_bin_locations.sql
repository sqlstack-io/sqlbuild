MODEL (
  description "One row per live warehouse bin, with aisle, bay and level parsed from its label",
  columns (
    bin_id (type VARCHAR, nullable false, audits [not_null, unique]),
    warehouse_code (type VARCHAR, audits [not_null]),
    bin_label (type VARCHAR),
    aisle (type VARCHAR),
    bay (type INT),
    shelf_level (type INT),
    is_pick_face (type BOOLEAN),
    max_weight_kg (type NUMBER(8, 2)),
    temperature_zone (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("inventory__bin_locations")
),

@sql.cdc_latest(src, src.key := (bin_id))

final AS (
  SELECT
    b.bin_id,
    UPPER(b.warehouse_code)::VARCHAR AS warehouse_code,
    UPPER(b.bin_label)::VARCHAR AS bin_label,
    REGEXP_SUBSTR(UPPER(b.bin_label), '^([A-Z]+)-', 1, 1, 'e', 1)::VARCHAR AS aisle,
    TRY_TO_NUMBER(REGEXP_SUBSTR(b.bin_label, '-([0-9]+)-', 1, 1, 'e', 1))::INT AS bay,
    TRY_TO_NUMBER(REGEXP_SUBSTR(b.bin_label, '-([0-9]+)$', 1, 1, 'e', 1))::INT AS shelf_level,
    COALESCE(b.attributes:pick_face::BOOLEAN, FALSE) AS is_pick_face,
    b.attributes:max_weight_kg::NUMBER(8, 2) AS max_weight_kg,
    COALESCE(b.attributes:temperature::VARIANT, 'ambient'::VARIANT)::VARCHAR AS temperature_zone,
    @sql.load_metadata(b)
  FROM cdc_latest AS b
)
SELECT *
FROM final
