MODEL (
  description """
    One row per check group and one grand-total row from GROUP BY ROLLUP, whose total row holds a
    NULL group code. Aggregates sit inside window functions in the grouped query: a grand total and
    a share, a RANK ordered by COUNT(*), and a QUALIFY that keeps the group with the largest SUM
  """,
  tags [edge_case],
  columns (
    group_label (type VARCHAR, nullable false, audits [not_null, unique]),
    group_code (type VARCHAR),
    group_amount (type NUMBER(38, 2)),
    check_count (type NUMBER(38, 0)),
    total_amount (type NUMBER(38, 2)),
    amount_share (type NUMBER(38, 6)),
    size_rank (type NUMBER(38, 0)),
    is_largest_group (type BOOLEAN),
  ),
);

WITH checks AS (
  SELECT
    c.group_code,
    c.amount
  FROM __ref("stg_edge_amount_checks") AS c
),

grouped AS (
  SELECT
    c.group_code,
    SUM(SUM(c.amount)) OVER () AS total_amount,
    DIV0(SUM(c.amount), SUM(SUM(c.amount)) OVER ()) AS amount_share,
    RANK() OVER (ORDER BY COUNT(*) DESC, c.group_code) AS size_rank
  FROM checks AS c
  GROUP BY
    c.group_code
),

largest AS (
  SELECT c.group_code
  FROM checks AS c
  GROUP BY
    c.group_code
  QUALIFY ROW_NUMBER() OVER (ORDER BY SUM(c.amount) DESC NULLS LAST, c.group_code) = 1
),

rolled_up AS (
  SELECT
    c.group_code,
    SUM(c.amount) AS group_amount,
    COUNT(*) AS check_count
  FROM checks AS c
  GROUP BY
    ROLLUP (
      c.group_code
    )
),

final AS (
  SELECT
    COALESCE(r.group_code, 'ALL')::VARCHAR AS group_label,
    r.group_code::VARCHAR AS group_code,
    r.group_amount::NUMBER(38, 2) AS group_amount,
    r.check_count::NUMBER(38, 0) AS check_count,
    g.total_amount::NUMBER(38, 2) AS total_amount,
    g.amount_share::NUMBER(38, 6) AS amount_share,
    g.size_rank::NUMBER(38, 0) AS size_rank,
    (
      l.group_code IS NOT NULL
    )::BOOLEAN AS is_largest_group
  FROM rolled_up AS r
  LEFT JOIN grouped AS g
    ON r.group_code = g.group_code
  LEFT JOIN largest AS l
    ON r.group_code = l.group_code
)

SELECT *
FROM final
