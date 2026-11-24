MODEL (
  description """
    One row per ISO week and top-level category with units, net sales and share of the week's units
  """,
  columns (
    basket_week_key (type VARCHAR, nullable false, audits [not_null, unique]),
    week_start (type DATE, audits [not_null]),
    category_level_1 (type VARCHAR),
    order_count (type INT),
    units (type INT),
    net_amount_minor (type INT),
    discounted_line_share (type NUMBER(9, 4)),
    unit_share_of_week (type NUMBER(9, 4)),
    category_rank (type INT),
  ),
);

WITH lines AS (
  SELECT *
  FROM __ref("fct_order_lines")
),

orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

weekly AS (
  SELECT
    DATE_TRUNC('week', o.placed_date_local) AS week_start,
    COALESCE(l.category_level_1, 'uncategorised') AS category_level_1,
    COUNT(DISTINCT l.order_id) AS order_count,
    SUM(l.quantity) AS units,
    SUM(l.net_amount_minor) AS net_amount_minor,
    AVG(IFF(l.discount_amount_minor > 0, 1, 0)) AS discounted_line_share
  FROM lines AS l
  INNER JOIN orders AS o
    ON l.order_id = o.order_id
  GROUP BY
    DATE_TRUNC('week', o.placed_date_local),
    COALESCE(l.category_level_1, 'uncategorised')
),

final AS (
  SELECT
    (
      TO_VARCHAR(w.week_start, 'YYYY-MM-DD') || ':' || w.category_level_1
    )::VARCHAR AS basket_week_key,
    w.week_start::DATE AS week_start,
    w.category_level_1::VARCHAR AS category_level_1,
    w.order_count::INT AS order_count,
    w.units::INT AS units,
    w.net_amount_minor::INT AS net_amount_minor,
    ROUND(w.discounted_line_share, 4)::NUMBER(9, 4) AS discounted_line_share,
    ROUND(
      RATIO_TO_REPORT(w.units) OVER (PARTITION BY w.week_start),
      4
    )::NUMBER(9, 4) AS unit_share_of_week,
    RANK() OVER (PARTITION BY w.week_start ORDER BY w.units DESC)::INT AS category_rank
  FROM weekly AS w
)

SELECT *
FROM final
