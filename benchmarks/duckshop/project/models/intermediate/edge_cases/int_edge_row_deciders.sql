MODEL (
  description """
    One row per basket picked by a clause whose columns decide which rows come back but are never
    output: WHERE, QUALIFY, GROUP BY 1 with HAVING, GROUP BY ALL, ORDER BY under LIMIT, EXISTS (as a
    filter and as an output), a FLATTEN whose input decides how many rows come out, GROUP BY ROLLUP
    by position, and EXCEPT ordered by position under LIMIT. Lineage that follows only the output
    columns misses every one of these
  """,
  tags [edge_case],
  columns (
    decider_row (type VARCHAR, nullable false, audits [not_null, unique]),
    decider (type VARCHAR),
    basket_id (type VARCHAR),
    detail (type VARCHAR),
  ),
);

WITH lines AS (
  SELECT
    l.line_id,
    l.basket_id,
    l.line_no,
    l.sku,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

baskets AS (
  SELECT DISTINCT l.basket_id
  FROM lines AS l
),

filtered AS (
  SELECT
    'where' AS decider,
    l.basket_id,
    l.line_id AS detail
  FROM lines AS l
  WHERE l.tag = 'bath'
),

first_lines AS (
  SELECT
    'qualify' AS decider,
    l.basket_id,
    l.line_id AS detail
  FROM lines AS l
  QUALIFY ROW_NUMBER() OVER (PARTITION BY l.basket_id ORDER BY l.line_no, l.line_id) = 1
),

busy AS (
  SELECT
    'group_by_position' AS decider,
    l.basket_id,
    NULL AS detail
  FROM lines AS l
  GROUP BY
    2
  HAVING COUNT(*) > 1
),

counted AS (
  SELECT
    'group_by_all' AS decider,
    l.basket_id,
    COUNT(*)::VARCHAR AS detail
  FROM lines AS l
  GROUP BY ALL
),

lowest_sku AS (
  SELECT
    'order_by_limit' AS decider,
    l.basket_id,
    l.line_id AS detail
  FROM lines AS l
  ORDER BY
    l.sku,
    l.line_id
  LIMIT 1
),

with_gift AS (
  SELECT
    'exists_filter' AS decider,
    b.basket_id,
    NULL AS detail
  FROM baskets AS b
  WHERE
    EXISTS(
      SELECT 1
      FROM lines AS g
      WHERE
        g.basket_id = b.basket_id
        AND g.tag = 'gift'
    )
),

gift_flags AS (
  SELECT
    'exists_output' AS decider,
    b.basket_id,
    (
      EXISTS(
        SELECT 1
        FROM lines AS g
        WHERE
          g.basket_id = b.basket_id
          AND g.tag = 'gift'
      )
    )::VARCHAR AS detail
  FROM baskets AS b
),

sku_parts AS (
  SELECT
    'flatten_input' AS decider,
    l.basket_id,
    l.line_id || '#' || f.index::VARCHAR AS detail
  FROM lines AS l, LATERAL FLATTEN(input => SPLIT(l.sku, '-')) AS f
),

rolled AS (
  SELECT
    'rollup_position' AS decider,
    l.basket_id,
    GROUPING(l.tag)::VARCHAR || ':' || COALESCE(l.tag, '~') || '=' || COUNT(*)::VARCHAR AS detail
  FROM lines AS l
  GROUP BY
    ROLLUP (
      2,
      l.tag
    )
),

without_pond AS (
  SELECT l.basket_id
  FROM lines AS l
  EXCEPT
  SELECT l.basket_id
  FROM lines AS l
  WHERE l.tag = 'pond'
ORDER BY 1 DESC
LIMIT 1
),

remaining AS (
  SELECT
    'except_limit' AS decider,
    w.basket_id,
    NULL AS detail
  FROM without_pond AS w
),

all_rows AS (
  SELECT *
  FROM filtered
  UNION ALL BY NAME
  SELECT *
  FROM first_lines
  UNION ALL BY NAME
  SELECT *
  FROM busy
  UNION ALL BY NAME
  SELECT *
  FROM counted
  UNION ALL BY NAME
  SELECT *
  FROM lowest_sku
  UNION ALL BY NAME
  SELECT *
  FROM with_gift
  UNION ALL BY NAME
  SELECT *
  FROM gift_flags
  UNION ALL BY NAME
  SELECT *
  FROM sku_parts
  UNION ALL BY NAME
  SELECT *
  FROM rolled
  UNION ALL BY NAME
  SELECT *
  FROM remaining
),

final AS (
  SELECT
    (
      a.decider || ':' || COALESCE(a.basket_id, '-') || ':' || COALESCE(a.detail, '-')
    )::VARCHAR AS decider_row,
    a.decider::VARCHAR AS decider,
    a.basket_id::VARCHAR AS basket_id,
    a.detail::VARCHAR AS detail
  FROM all_rows AS a
)

SELECT *
FROM final
