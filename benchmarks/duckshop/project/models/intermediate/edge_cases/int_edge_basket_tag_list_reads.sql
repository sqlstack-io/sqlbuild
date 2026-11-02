MODEL (
  description """
    One row per basket with reads of its tags in line order: overlap with a fixed list, distinct
    tags, the position and count of one tag, the tags upper-cased, a time read from the basket
    id and the size of a bracket array literal
  """,
  tags [edge_case],
  columns (
    basket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    has_colour (type BOOLEAN),
    distinct_tag_count (type NUMBER(38, 0)),
    red_position (type NUMBER(38, 0)),
    non_red_count (type NUMBER(38, 0)),
    upper_tags (type VARCHAR),
    basket_time (type TIME),
    pair_size (type NUMBER(38, 0)),
  ),
);

WITH tag_lists AS (
  SELECT
    l.basket_id,
    ARRAY_AGG(l.tag) WITHIN GROUP (ORDER BY l.line_no) AS tags
  FROM __ref("stg_edge_basket_lines") AS l
  GROUP BY
    l.basket_id
),

final AS (
  SELECT
    t.basket_id,
    ARRAYS_OVERLAP(t.tags, ['red', 'blue'])::BOOLEAN AS has_colour,
    ARRAY_SIZE(ARRAY_DISTINCT(t.tags))::NUMBER(38, 0) AS distinct_tag_count,
    ARRAY_POSITION('red'::VARIANT, t.tags)::NUMBER(38, 0) AS red_position,
    ARRAY_SIZE(ARRAY_REMOVE(t.tags, 'red'::VARIANT))::NUMBER(38, 0) AS non_red_count,
    ARRAY_TO_STRING(TRANSFORM(t.tags, tag -> UPPER(tag)), ',')::VARCHAR AS upper_tags,
    TRY_TO_TIME(t.basket_id)::TIME AS basket_time,
    ARRAY_SIZE([t.basket_id, ARRAY_SIZE(t.tags)])::NUMBER(38, 0) AS pair_size
  FROM tag_lists AS t
)

SELECT *
FROM final
