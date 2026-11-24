MODEL (
  description """
    One row per distinct pair of tags seen together in a basket, written in both orders. Arrays keep
    their element order, so SELECT DISTINCT keeps [bath, gift] and [gift, bath] apart and merges
    only pairs repeated in the same order; a NULL tag stays an undefined element
  """,
  tags [edge_case],
  columns (
    tag_pair_text (type VARCHAR, nullable false, audits [not_null, unique]),
    tag_pair (type ARRAY),
    pair_size (type NUMBER(38, 0)),
  ),
);

WITH lines AS (
  SELECT
    l.basket_id,
    l.line_no,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

pairs AS (
  SELECT ARRAY_CONSTRUCT(a.tag, b.tag) AS tag_pair
  FROM lines AS a
  INNER JOIN lines AS b
    ON a.basket_id = b.basket_id
    AND a.line_no < b.line_no
  UNION ALL
  SELECT ARRAY_CONSTRUCT(b.tag, a.tag) AS tag_pair
  FROM lines AS a
  INNER JOIN lines AS b
    ON a.basket_id = b.basket_id
    AND a.line_no < b.line_no
),

distinct_pairs AS (
  SELECT DISTINCT p.tag_pair
  FROM pairs AS p
),

final AS (
  SELECT
    ARRAY_TO_STRING(d.tag_pair, '|')::VARCHAR AS tag_pair_text,
    d.tag_pair::ARRAY AS tag_pair,
    ARRAY_SIZE(d.tag_pair)::NUMBER(38, 0) AS pair_size
  FROM distinct_pairs AS d
)

SELECT *
FROM final
