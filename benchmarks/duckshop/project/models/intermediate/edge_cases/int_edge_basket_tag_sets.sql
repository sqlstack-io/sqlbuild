MODEL (
  description """
    One row per basket with order-free reads of an unordered ARRAY_AGG of its tags. Snowflake does
    not define the element order of an ARRAY_AGG without WITHIN GROUP, so the model outputs only
    facts that hold for every order: sizes, membership, overlaps, sorted distinct sets and the
    values FLATTEN reads back through a CTE
  """,
  tags [edge_case],
  columns (
    basket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    tag_count (type NUMBER(38, 0)),
    has_gift (type BOOLEAN),
    overlaps_pond_or_garden (type BOOLEAN),
    distinct_tag_count (type NUMBER(38, 0)),
    distinct_tags (type ARRAY),
    flattened_count (type NUMBER(38, 0)),
    flattened_tags (type VARCHAR),
  ),
);

WITH lines AS (
  SELECT
    l.basket_id,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

baskets AS (
  -- Unordered on purpose: every read below gives the same answer for any element order.
  SELECT
    l.basket_id,
    ARRAY_AGG(l.tag) AS tags
  FROM lines AS l
  GROUP BY
    l.basket_id
),

flattened AS (
  SELECT
    b.basket_id,
    f.value::VARCHAR AS tag
  FROM baskets AS b, LATERAL FLATTEN(input => b.tags) AS f
),

flattened_tags AS (
  SELECT
    f.basket_id,
    COUNT(*) AS flattened_count,
    LISTAGG(DISTINCT f.tag, ',') WITHIN GROUP (ORDER BY f.tag) AS flattened_tags
  FROM flattened AS f
  GROUP BY
    f.basket_id
),

final AS (
  SELECT
    b.basket_id,
    ARRAY_SIZE(b.tags)::NUMBER(38, 0) AS tag_count,
    ARRAY_CONTAINS('gift'::VARIANT, b.tags)::BOOLEAN AS has_gift,
    ARRAYS_OVERLAP(b.tags, ARRAY_CONSTRUCT('pond', 'garden'))::BOOLEAN AS overlaps_pond_or_garden,
    ARRAY_SIZE(
      ARRAY_DISTINCT(ARRAY_CAT(b.tags, ARRAY_CONSTRUCT('gift')))
    )::NUMBER(38, 0) AS distinct_tag_count,
    ARRAY_SORT(ARRAY_DISTINCT(ARRAY_CAT(b.tags, ARRAY_CONSTRUCT('gift'))))::ARRAY AS distinct_tags,
    COALESCE(t.flattened_count, 0)::NUMBER(38, 0) AS flattened_count,
    t.flattened_tags::VARCHAR AS flattened_tags
  FROM baskets AS b
  LEFT JOIN flattened_tags AS t
    ON b.basket_id = t.basket_id
)

SELECT *
FROM final
