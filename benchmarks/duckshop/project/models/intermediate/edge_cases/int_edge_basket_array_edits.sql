MODEL (
  description """
    One row per basket with its ordered SKU list edited by array functions: ARRAY_APPEND,
    ARRAY_INTERSECTION with a fixed list, ARRAY_SLICE with positive and negative bounds, TO_ARRAY of
    a single tag and of a NULL, and an array written with brackets
  """,
  tags [edge_case],
  columns (
    basket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    skus (type ARRAY),
    skus_with_bag (type ARRAY),
    common_skus (type ARRAY),
    first_two_skus (type ARRAY),
    all_but_last_sku (type ARRAY),
    first_tag_array (type ARRAY),
    missing_tag_array (type ARRAY),
    bracket_pair (type ARRAY),
  ),
);

WITH lines AS (
  SELECT
    l.basket_id,
    l.line_no,
    l.sku,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

baskets AS (
  SELECT
    l.basket_id,
    ARRAY_AGG(l.sku) WITHIN GROUP (ORDER BY l.line_no, l.sku) AS skus,
    MIN_BY(l.tag, l.line_no) AS first_tag
  FROM lines AS l
  GROUP BY
    l.basket_id
),

final AS (
  SELECT
    b.basket_id,
    b.skus::ARRAY AS skus,
    ARRAY_APPEND(b.skus, 'BAG-00')::ARRAY AS skus_with_bag,
    ARRAY_SORT(
      ARRAY_INTERSECTION(b.skus, ARRAY_CONSTRUCT('MALLARD-01', 'EIDER-03', 'SWAN-09'))
    )::ARRAY AS common_skus,
    ARRAY_SLICE(b.skus, 0, 2)::ARRAY AS first_two_skus,
    ARRAY_SLICE(b.skus, 0, -1)::ARRAY AS all_but_last_sku,
    TO_ARRAY(b.first_tag)::ARRAY AS first_tag_array,
    TO_ARRAY(NULL)::ARRAY AS missing_tag_array,
    [b.basket_id, ARRAY_SIZE(b.skus)]::ARRAY AS bracket_pair
  FROM baskets AS b
)

SELECT *
FROM final
