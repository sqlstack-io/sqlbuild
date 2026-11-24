MODEL (
  description """
    The basket whose unordered tag array sorts last, picked by ORDER BY ... DESC LIMIT 1 written by
    position, by name and through IFF, over a UNION ALL, and the first line by a positional ORDER BY
    over a constructed array. How Snowflake ranks an ARRAY_AGG depends on its element order, which
    it does not define; the fixtures give every basket a different set of first elements, so the
    pick is the same for every element order
  """,
  tags [edge_case],
  columns (
    pick_kind (type VARCHAR, nullable false, audits [not_null, unique]),
    picked_id (type VARCHAR),
    picked_size (type NUMBER(38, 0)),
  ),
);

WITH lines AS (
  SELECT
    l.line_id,
    l.basket_id,
    l.sku,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

baskets AS (
  SELECT
    l.basket_id,
    ARRAY_AGG(l.tag) AS tags
  FROM lines AS l
  GROUP BY
    l.basket_id
),

by_position AS (
  SELECT
    b.basket_id,
    b.tags
  FROM baskets AS b
  ORDER BY
    2 DESC
  LIMIT 1
),

by_name AS (
  SELECT b.basket_id
  FROM baskets AS b
  ORDER BY
    b.tags DESC
  LIMIT 1
),

through_iff AS (
  SELECT
    b.basket_id,
    IFF(TRUE, b.tags, b.tags) AS tags
  FROM baskets AS b
  ORDER BY
    2 DESC
  LIMIT 1
),

over_union AS (
  SELECT
    b.basket_id,
    b.tags
  FROM baskets AS b
  UNION ALL
  SELECT
    'B-0' AS basket_id,
    ARRAY_CONSTRUCT('aaa') AS tags
ORDER BY 2 DESC
LIMIT 1
),

constructed AS (
  SELECT
    l.line_id,
    ARRAY_CONSTRUCT(l.sku) AS skus
  FROM lines AS l
  ORDER BY
    2,
    1
  LIMIT 1
),

final AS (
  SELECT
    'by_position' AS pick_kind,
    p.basket_id AS picked_id,
    ARRAY_SIZE(p.tags)::NUMBER(38, 0) AS picked_size
  FROM by_position AS p
  UNION ALL
  SELECT
    'by_name' AS pick_kind,
    n.basket_id AS picked_id,
    NULL::NUMBER(38, 0) AS picked_size
  FROM by_name AS n
  UNION ALL
  SELECT
    'through_iff' AS pick_kind,
    i.basket_id AS picked_id,
    ARRAY_SIZE(i.tags)::NUMBER(38, 0) AS picked_size
  FROM through_iff AS i
  UNION ALL
  SELECT
    'over_union' AS pick_kind,
    u.basket_id AS picked_id,
    ARRAY_SIZE(u.tags)::NUMBER(38, 0) AS picked_size
  FROM over_union AS u
  UNION ALL
  SELECT
    'constructed' AS pick_kind,
    c.line_id AS picked_id,
    ARRAY_SIZE(c.skus)::NUMBER(38, 0) AS picked_size
  FROM constructed AS c
)

SELECT *
FROM final
