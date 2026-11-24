MODEL (
  description """
    One row per barcode of a variant, from the variant's barcode array, with its symbology inferred
    from its length. Barcodes listed on several variants are dropped as ambiguous.
  """,
  columns (
    barcode (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    symbology (type VARCHAR, audits [accepted_values (values ["EAN13", "UPCA", "EAN8", "OTHER"])]),
    is_primary (type BOOLEAN),
  ),
);

WITH variants AS (
  SELECT *
  FROM __ref("stg_product_variants")
),

array_barcodes AS (
  SELECT
    v.sku,
    b.index::INT AS barcode_index,
    REGEXP_REPLACE(
      b.value::VARCHAR,
      '[^0-9]',
      ''
    ) AS digits
  FROM variants AS v, LATERAL FLATTEN(input => v.barcodes) AS b
),

unambiguous AS (
  SELECT
    a.sku,
    a.digits,
    MIN(a.barcode_index) AS barcode_index
  FROM array_barcodes AS a
  WHERE a.digits <> ''
  GROUP BY
    a.sku,
    a.digits
  QUALIFY COUNT(*) OVER (PARTITION BY a.digits) = 1
),

final AS (
  SELECT
    u.digits::VARCHAR AS barcode,
    u.sku,
    CASE LENGTH(u.digits)
      WHEN 13
      THEN 'EAN13'
      WHEN 12
      THEN 'UPCA'
      WHEN 8
      THEN 'EAN8'
      ELSE 'OTHER'
    END::VARCHAR AS symbology,
    (
      u.barcode_index = 0
    ) AS is_primary
  FROM unambiguous AS u
)

SELECT *
FROM final
