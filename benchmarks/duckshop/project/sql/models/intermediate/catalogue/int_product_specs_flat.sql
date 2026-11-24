MODEL (
  description """
    One row per product with its specification object flattened: the most common spec keys pivoted
    into typed columns, unit-normalised dimensions, and the remaining keys re-aggregated into an
    object.
  """,
  columns (
    product_id (type VARCHAR, nullable false, audits [not_null, unique]),
    material (type VARCHAR),
    colour_family (type VARCHAR),
    width_cm (type NUMBER(10, 2)),
    height_cm (type NUMBER(10, 2)),
    capacity_ml (type INT),
    is_dishwasher_safe (type BOOLEAN),
    spec_value_types (type ARRAY),
    other_specs (type OBJECT),
    numeric_spec_count (type INT),
  ),
);

WITH products AS (
  SELECT *
  FROM __ref("stg_products")
),

spec_entries AS (
  SELECT
    p.product_id,
    LOWER(REGEXP_REPLACE(
      k.value::VARCHAR,
      '[^A-Za-z0-9]+',
      '_'
    )) AS spec_key,
    GET(p.specs, k.value::VARCHAR) AS spec_value
  FROM products AS p, LATERAL FLATTEN(input => OBJECT_KEYS(p.specs)) AS k
),

typed_entries AS (
  SELECT
    e.product_id,
    e.spec_key,
    e.spec_value,
    TYPEOF(e.spec_value) AS value_type,
    CASE
      WHEN IS_DECIMAL(e.spec_value)
      OR IS_INTEGER(e.spec_value)
      THEN e.spec_value::NUMBER(18, 4)
      WHEN REGEXP_LIKE(
        e.spec_value::VARCHAR,
        '^\\s*[0-9]+(\\.[0-9]+)?\\s*(mm|cm|m)\\s*$',
        'i'
      )
      THEN TO_NUMBER(REGEXP_SUBSTR(e.spec_value::VARCHAR, '[0-9]+(\\.[0-9]+)?'), 18, 4)
      * DECODE(
        LOWER(REGEXP_SUBSTR(
          e.spec_value::VARCHAR,
          '(mm|cm|m)',
          1,
          1,
          'i'
        )),
        'mm', 0.1,
        'm', 100,
        1
      )
    END AS numeric_value
  FROM spec_entries AS e
),

pivot_source AS (
  SELECT
    t.product_id,
    t.spec_key,
    t.spec_value
  FROM typed_entries AS t
  WHERE t.spec_key IN ('material', 'colour_family', 'dishwasher_safe')
),

pivoted AS (
  SELECT
    p.product_id,
    p.material,
    p.colour_family,
    p.dishwasher_safe
  FROM pivot_source PIVOT(
    MAX(spec_value) FOR spec_key IN ('material', 'colour_family', 'dishwasher_safe')
  ) AS p(product_id, material, colour_family, dishwasher_safe)
),

dimensions AS (
  SELECT
    t.product_id,
    MAX(IFF(t.spec_key IN ('width', 'width_cm'), t.numeric_value, NULL)) AS width_cm,
    MAX(IFF(t.spec_key IN ('height', 'height_cm'), t.numeric_value, NULL)) AS height_cm,
    MAX(IFF(t.spec_key IN ('capacity', 'capacity_ml'), t.numeric_value, NULL)) AS capacity_ml,
    COUNT(t.numeric_value) AS numeric_spec_count,
    ARRAY_AGG(DISTINCT t.value_type) WITHIN GROUP (ORDER BY t.value_type) AS spec_value_types
  FROM typed_entries AS t
  GROUP BY
    t.product_id
),

leftovers AS (
  SELECT
    t.product_id,
    OBJECT_AGG(t.spec_key, t.spec_value) AS other_specs
  FROM typed_entries AS t
  WHERE
    t.spec_key NOT IN (
      'material',
      'colour_family',
      'dishwasher_safe',
      'width',
      'width_cm',
      'height',
      'height_cm',
      'capacity',
      'capacity_ml'
    )
  GROUP BY
    t.product_id
),

final AS (
  SELECT
    p.product_id,
    INITCAP(pv.material::VARCHAR)::VARCHAR AS material,
    LOWER(pv.colour_family::VARCHAR)::VARCHAR AS colour_family,
    ROUND(d.width_cm, 2)::NUMBER(10, 2) AS width_cm,
    ROUND(d.height_cm, 2)::NUMBER(10, 2) AS height_cm,
    d.capacity_ml::INT AS capacity_ml,
    TRY_TO_BOOLEAN(pv.dishwasher_safe::VARCHAR)::BOOLEAN AS is_dishwasher_safe,
    COALESCE(d.spec_value_types, ARRAY_CONSTRUCT())::ARRAY AS spec_value_types,
    COALESCE(l.other_specs, OBJECT_CONSTRUCT())::OBJECT AS other_specs,
    COALESCE(d.numeric_spec_count, 0)::INT AS numeric_spec_count
  FROM products AS p
  LEFT JOIN pivoted AS pv
    ON p.product_id = pv.product_id
  LEFT JOIN dimensions AS d
    ON p.product_id = d.product_id
  LEFT JOIN leftovers AS l
    ON p.product_id = l.product_id
)

SELECT *
FROM final
