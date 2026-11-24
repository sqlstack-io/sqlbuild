MODEL (
  description """
    One row per basket built from valid Snowflake SQL that name and type checks have reported as
    errors: LAST_DAY with a date part as its second argument, an aggregate aliased as the column it
    reads, positional columns, columns read by the names Snowflake gives unaliased expressions, and
    contracts that declare TEXT(5) and a NUMBER(38,0) fed by COUNT
  """,
  tags [edge_case],
  columns (
    basket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    month_end (type DATE),
    line_no (type NUMBER(38, 0)),
    positional_sku (type VARCHAR),
    next_line_no (type NUMBER(38, 0)),
    line_no_text (type VARCHAR),
    sku_prefix (type TEXT(5)),
    line_count (type NUMBER(38, 0)),
  ),
);

WITH lines AS (
  SELECT
    l.basket_id,
    l.sku,
    l.line_no
  FROM __ref("stg_edge_basket_lines") AS l
),

dated AS (
  SELECT MIN(TRY_TO_DATE(d.date_text, 'YYYY-MM-DD')) AS first_date
  FROM __ref("stg_edge_date_texts") AS d
),

own_names AS (
  SELECT
    l.basket_id,
    MAX(l.line_no) AS line_no
  FROM lines AS l
  GROUP BY
    l.basket_id
),

positional AS (
  SELECT
    $1 AS basket_id,
    MIN($2) AS positional_sku
  FROM lines
  GROUP BY
    1
),

unnamed AS (
  -- No aliases: Snowflake names these columns LINE_NO + 1 and LINE_NO::VARCHAR.
  SELECT
    basket_id,
    line_no + 1,
    line_no::VARCHAR
  FROM lines
),

unnamed_per_basket AS (
  SELECT
    u.basket_id,
    MAX(u."LINE_NO + 1") AS next_line_no,
    MIN(u."LINE_NO::VARCHAR") AS line_no_text
  FROM unnamed AS u
  GROUP BY
    u.basket_id
),

counted AS (
  SELECT
    l.basket_id,
    LEFT(MIN(l.sku), 5)::VARCHAR(5) AS sku_prefix,
    COUNT(*) AS line_count
  FROM lines AS l
  GROUP BY
    l.basket_id
),

final AS (
  SELECT
    c.basket_id,
    LAST_DAY(d.first_date, month)::DATE AS month_end,
    n.line_no,
    p.positional_sku::VARCHAR AS positional_sku,
    u.next_line_no::NUMBER(38, 0) AS next_line_no,
    u.line_no_text::VARCHAR AS line_no_text,
    c.sku_prefix,
    c.line_count
  FROM counted AS c
  CROSS JOIN dated AS d
  LEFT JOIN own_names AS n
    ON c.basket_id = n.basket_id
  LEFT JOIN positional AS p
    ON c.basket_id = p.basket_id
  LEFT JOIN unnamed_per_basket AS u
    ON c.basket_id = u.basket_id
)

SELECT *
FROM final
