MODEL (
  description """
    One row per return with its free-text reason classified into the return_reason enum by pattern
  """,
  columns (
    return_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR),
    sku (type VARCHAR),
    qty (type INT),
    reason_text (type VARCHAR),
    return_reason (type return_reason),
    classified_by (type VARCHAR),
    requested_at (type TIMESTAMP_TZ),
    category_level_1 (type VARCHAR),
    ordered_qty (type INT),
    is_over_return (type BOOLEAN),
    days_after_order (type INT),
  ),
);

WITH returns AS (
  SELECT *
  FROM __ref("stg_returns")
),

order_lines AS (
  SELECT *
  FROM __ref("stg_order_lines")
),

orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

catalogue AS (
  SELECT *
  FROM __ref("int_product_catalogue")
),

ordered_quantities AS (
  SELECT
    l.order_id,
    l.sku,
    SUM(l.quantity) AS ordered_qty
  FROM order_lines AS l
  GROUP BY
    l.order_id,
    l.sku
),

normalised AS (
  SELECT
    r.return_id,
    r.order_id,
    r.sku,
    r.qty,
    r.reason_text,
    r.requested_at,
    LOWER(REGEXP_REPLACE(
      COALESCE(r.reason_text, ''),
      '[^A-Za-z ]',
      ' '
    )) AS reason_words
  FROM returns AS r
),

with_order AS (
  SELECT
    n.return_id,
    n.order_id,
    n.sku,
    n.qty,
    n.reason_text,
    n.requested_at,
    n.reason_words,
    c.category_level_1,
    q.ordered_qty,
    o.placed_at
  FROM normalised AS n
  LEFT JOIN catalogue AS c
    ON n.sku = c.sku
  LEFT JOIN ordered_quantities AS q
    ON n.order_id = q.order_id
    AND n.sku = q.sku
  LEFT JOIN orders AS o
    ON n.order_id = o.order_id
),

final AS (
  SELECT
    n.return_id,
    n.order_id,
    n.sku,
    n.qty,
    n.reason_text,
    CASE
      WHEN REGEXP_LIKE(n.reason_words, '.*(damag|broken|crack|dent|scratch).*')
      THEN @enum("return_reason").DAMAGED
      WHEN REGEXP_LIKE(
        n.reason_words,
        '.*(wrong item|wrong colou?r|not what i ordered|different).*'
      )
      THEN @enum("return_reason").WRONG_ITEM
      WHEN REGEXP_LIKE(n.reason_words, '.*(too (big|small|large)|size|fit).*')
      THEN @enum("return_reason").SIZE_FIT
      WHEN REGEXP_LIKE(n.reason_words, '.*(not as described|quality|looks different|photo).*')
      THEN @enum("return_reason").NOT_AS_DESCRIBED
      WHEN REGEXP_LIKE(n.reason_words, '.*(changed my mind|no longer|don t need|dont need).*')
      THEN @enum("return_reason").CHANGED_MIND
      WHEN REGEXP_LIKE(n.reason_words, '.*(late|delay|too long).*')
      THEN @enum("return_reason").LATE_DELIVERY
      ELSE @enum("return_reason").OTHER
    END::VARCHAR AS return_reason,
    IFF(TRIM(n.reason_words) = '', 'default', 'pattern')::VARCHAR AS classified_by,
    n.requested_at,
    n.category_level_1::VARCHAR AS category_level_1,
    n.ordered_qty::INT AS ordered_qty,
    COALESCE(n.qty > n.ordered_qty, TRUE)::BOOLEAN AS is_over_return,
    DATEDIFF('day', n.placed_at, n.requested_at)::INT AS days_after_order
  FROM with_order AS n
)

SELECT *
FROM final
